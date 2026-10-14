//! The local library: the user's own songs, playlists and play history.
//!
//! This is the data layer the aggregator design's 我的歌 / 歌单 / 最近播放 pages
//! read from. Two ideas shape it:
//!
//! * A song appears **once**. A [`LibraryEntry`] is one recording; the platform
//!   copies of it are `versions`, and [`identity`] decides what counts as the
//!   same recording.
//! * A platform collection is somebody else's data. Reading it is a
//!   capability, writing to it is not: only a [`PlaylistOrigin::Local`]
//!   playlist is ever `readonly == false`.
//!
//! It is stored in its own file, `library.json`, rather than inside
//! `config.json`. The config holds login credentials and is written through
//! `platform::write_private` for that reason; a library is incidental user data
//! that can grow to hundreds of entries, and mixing the two would mean every
//! saved song rewrites the credentials file.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::api::models::{Source, Track};
use crate::platform;
use crate::util;

pub mod identity;

pub use identity::{sameness, verdict, Verdict, MERGE_THRESHOLD};

/// How many recent plays are remembered. Far more than any list shows; it is a
/// history, not a view, so the cap only stops the file growing without bound.
pub const MAX_RECENT: usize = 200;

/// The document version, so a future migration can tell an old file apart.
pub const LIBRARY_VERSION: u32 = 1;

/// One song in the library, with every platform copy of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// The preferred version — the one the design calls "the copy that is
    /// playing". Which copy actually plays is decided at resolve time by
    /// [`LocalLibrary::preferred`], so this is the first one that was added.
    pub track: Track,
    /// Every known platform version of this recording.
    ///
    /// Includes [`LibraryEntry::track`] as its first element, so iterating
    /// `versions` sees the preferred copy too.
    #[serde(default)]
    pub versions: Vec<Track>,
    /// When the song was added, as `2026-08-14`.
    ///
    /// A plain date string on purpose: the design shows "8 月 14 日", and
    /// formatting that is the UI's job. Keeping it textual also keeps this
    /// module free of a date/time dependency.
    pub added_at: String,
    /// Ids of the playlists this song belongs to. Kept on the entry as well as
    /// in [`Playlist::entries`] so a row can be filtered without scanning every
    /// playlist.
    #[serde(default)]
    pub playlists: Vec<String>,
    /// A version the listener chose by hand, as [`Track::key`].
    ///
    /// [`preferred`](LocalLibrary::preferred) ranks candidates by quality and
    /// whether they are downloaded; a pin outranks all of that. Without it the
    /// "改用这一版" button in 来源详情 would be a suggestion the rules could
    /// overrule on the next resolve, which is not what pressing it means.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<String>,
}

/// Where a playlist comes from, and therefore whether it can be edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistOrigin {
    /// Created here. The only writable origin.
    Local,
    /// Synced from a platform collection (a Bilibili favourite folder). Read
    /// only: the source of truth is the platform.
    Platform(Source),
}

/// One playlist: the design's local list, or a platform collection synced in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub origin: PlaylistOrigin,
    /// The design's "read/write declaration": `false` for a playlist created
    /// here, `true` for one synced from a platform. `true` means the interface
    /// must not offer editing, because the next sync would undo it.
    pub readonly: bool,
    /// Members, as [`LocalLibrary::key_of`] keys rather than `Track`s.
    ///
    /// Keys survive a re-resolve or a cover change, and the same song is then
    /// referenced by both the playlist and [`LibraryEntry`] without duplicating
    /// the track document.
    #[serde(default)]
    pub entries: Vec<String>,
}

impl Playlist {
    /// A playlist created in this app: writable, and the only kind that is.
    pub fn local(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            origin: PlaylistOrigin::Local,
            readonly: false,
            entries: Vec::new(),
        }
    }

    /// A playlist synced from a platform collection.
    ///
    /// Always `readonly`: the platform owns the membership, and the app only
    /// mirrors it. Constructed here rather than by hand so the invariant cannot
    /// be forgotten at a call site.
    pub fn platform(id: impl Into<String>, title: impl Into<String>, source: Source) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            origin: PlaylistOrigin::Platform(source),
            readonly: true,
            entries: Vec::new(),
        }
    }

    /// Whether the interface may offer to edit this playlist.
    pub fn is_writable(&self) -> bool {
        !self.readonly
    }
}

/// What the last sync of a platform collection changed.
///
/// Counted from the difference between the list that was there before and the one
/// the platform just gave back — which is the only honest source for "新增 6 首，
/// 移除 1 首": nothing else in the app knows what used to be in somebody else's
/// favourites folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncReport {
    /// `2026-08-14 20:41`, UTC. Readable in the file; the interface shows an age
    /// instead (see `at_unix`).
    pub at: String,
    /// The same instant, so the interface can show how long ago it was.
    ///
    /// An age is the one reading that survives the lack of a timezone: it is a
    /// difference of two UTC instants, so it cannot be an hour or two out the way
    /// a clock read out of a UTC stamp would be. An old file without this field
    /// simply shows no age.
    #[serde(default)]
    pub at_unix: u64,
    pub added: usize,
    pub removed: usize,
    /// Songs that exist on more than one platform, so they cost one row, not two.
    pub merged: usize,
}

/// One play, for the 最近播放 page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentPlay {
    /// A [`LocalLibrary::key_of`] key.
    pub key: String,
    /// When it was played, as `2026-08-14 21:03`. The design shows a relative
    /// label, so only the instant matters, not a presentation format.
    pub at: String,
}

/// The persisted library document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalLibrary {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: Vec<LibraryEntry>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
    #[serde(default)]
    pub recent: Vec<RecentPlay>,
    /// The last platform sync, for 来源与同步's meters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<SyncReport>,
}

fn default_version() -> u32 {
    LIBRARY_VERSION
}

impl Default for LocalLibrary {
    fn default() -> Self {
        Self {
            version: LIBRARY_VERSION,
            entries: Vec::new(),
            playlists: Vec::new(),
            recent: Vec::new(),
            last_sync: None,
        }
    }
}

impl LocalLibrary {
    /// `library.json`, beside `config.json` but deliberately not inside it.
    pub fn path() -> PathBuf {
        platform::config_dir().join("library.json")
    }

    /// Read the library, never failing.
    ///
    /// A missing file is a fresh install and a corrupt one is a bad write; both
    /// yield the default and a line on stderr, because a data file must not be
    /// able to stop the application from starting. This mirrors [`Config::load`].
    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    /// Write the library through `platform::write_private`, like every other
    /// file under the config directory.
    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Self::path())
    }

    /// The identity of a library entry.
    ///
    /// Deliberately **not** [`Track::key`]: that one is built to avoid a
    /// cross-platform collision, which is exactly wrong here — the whole point
    /// of the library is that a Bilibili copy and a YouTube copy of one song are
    /// one row.
    ///
    /// The key is "cleaned title | primary artist | duration bucket":
    ///
    /// * the title goes through [`util::clean_song_title`], so `【4K修复】` does
    ///   not produce a second key for the same song;
    /// * the artist is `artist` only. `author` is an uploader, so including it
    ///   would give the same song a different key on every re-upload platform;
    /// * the duration is bucketed by five seconds, because the two platforms
    ///   report the same recording a second or two apart. Five seconds admits
    ///   that drift while still separating a single from a long version.
    pub fn key_of(track: &Track) -> String {
        let title = normalize_for_key(&util::clean_song_title(&track.title));
        let artist = track
            .artist
            .as_deref()
            .map(normalize_for_key)
            .unwrap_or_default();
        format!("{title}|{artist}|{}", duration_bucket(track.duration))
    }

    /// Add a track, merging it into an existing entry when it is the same song.
    ///
    /// Returns `true` only when a new entry was created. A second platform copy
    /// lands in [`LibraryEntry::versions`] and returns `false`; adding the very
    /// same platform copy twice changes nothing at all, because two copies of
    /// one video are not two versions.
    ///
    /// Matching is by [`key_of`](Self::key_of) first (a direct hit, cheap) and
    /// then by [`identity::sameness`], so a copy whose duration bucket differs
    /// but which is recognisably the same recording still merges.
    pub fn add(&mut self, track: Track) -> bool {
        let key = Self::key_of(&track);
        let existing = self.entries.iter_mut().find(|entry| {
            Self::key_of(&entry.track) == key
                || entry.versions.iter().any(|v| Self::key_of(v) == key)
                || identity::sameness(&entry.track, &track) >= MERGE_THRESHOLD
        });

        match existing {
            Some(entry) => {
                // One version per platform copy. `Track::key` is the right tool
                // here: it *is* per-platform identity.
                if !entry.versions.iter().any(|v| v.key() == track.key()) {
                    entry.versions.push(track);
                }
                false
            }
            None => {
                self.entries.push(LibraryEntry {
                    versions: vec![track.clone()],
                    track,
                    added_at: today(),
                    playlists: Vec::new(),
                    pinned: None,
                });
                true
            }
        }
    }

    /// Find the entry a key belongs to, by preferred or any known version.
    pub fn find(&self, key: &str) -> Option<&LibraryEntry> {
        self.entries.iter().find(|entry| entry_matches(entry, key))
    }

    /// [`find`](Self::find) with a mutable entry, for editing membership.
    pub fn find_mut(&mut self, key: &str) -> Option<&mut LibraryEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry_matches(entry, key))
    }

    /// Remove an entry, returning whether anything was removed.
    ///
    /// Also drops the key from every playlist and from the recent list: leaving
    /// a dangling member would make a playlist show a row that cannot resolve.
    pub fn remove(&mut self, key: &str) -> bool {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry_matches(entry, key))
        else {
            return false;
        };
        self.entries.remove(index);
        for playlist in &mut self.playlists {
            playlist.entries.retain(|member| member != key);
        }
        self.recent.retain(|play| play.key != key);
        true
    }

    /// Record a play: newest first, one entry per song, capped at
    /// [`MAX_RECENT`].
    ///
    /// Playing an old song again moves it to the top instead of adding a second
    /// row, which is what makes 最近播放 a list of songs rather than of events.
    pub fn record_play(&mut self, key: &str) {
        if key.trim().is_empty() {
            return;
        }
        self.recent.retain(|play| play.key != key);
        self.recent.insert(
            0,
            RecentPlay {
                key: key.to_owned(),
                at: local_timestamp(unix_seconds()),
            },
        );
        self.recent.truncate(MAX_RECENT);
    }

    /// Replaces a platform collection's membership, reporting what changed.
    ///
    /// The diff is the point: a sync is only interesting for what it added and
    /// what somebody removed on the platform's side, and both are computed here
    /// rather than being guessed at by the interface.
    pub fn sync_playlist(
        &mut self,
        id: &str,
        title: &str,
        source: Source,
        entries: Vec<String>,
    ) -> SyncReport {
        let previous: Vec<String> = self
            .playlists
            .iter()
            .find(|playlist| playlist.id == id)
            .map(|playlist| playlist.entries.clone())
            .unwrap_or_default();
        let added = entries.iter().filter(|key| !previous.contains(key)).count();
        let removed = previous.iter().filter(|key| !entries.contains(key)).count();

        let mut playlist = Playlist::platform(id, title, source);
        playlist.entries = entries;
        match self.playlists.iter_mut().find(|existing| existing.id == id) {
            Some(existing) => {
                existing.title = playlist.title;
                existing.origin = playlist.origin;
                existing.readonly = playlist.readonly;
                existing.entries = playlist.entries;
            }
            None => self.playlists.push(playlist),
        }

        let now = unix_seconds();
        let report = SyncReport {
            at: local_timestamp(now),
            at_unix: now,
            added,
            removed,
            merged: self
                .entries
                .iter()
                .filter(|entry| entry.versions.len() > 1)
                .count(),
        };
        self.last_sync = Some(report.clone());
        report
    }

    /// Pins a version of a song, or clears the pin with `None`.
    ///
    /// Returns whether the entry exists. Pinning a version the entry does not
    /// hold is refused: it would be a preference for something that is not there.
    pub fn pin(&mut self, key: &str, version: Option<String>) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| Self::key_of(&entry.track) == key)
        else {
            return false;
        };
        if let Some(version) = &version {
            let known = entry.versions.iter().any(|track| track.key() == *version)
                || entry.track.key() == *version;
            if !known {
                return false;
            }
        }
        entry.pinned = version;
        true
    }

    /// The version that should play for `key`.
    ///
    /// Convenience over [`preferred`](Self::preferred) for the common case where
    /// the caller has a library key in hand.
    pub fn resolve<'a>(
        &'a self,
        key: &str,
        cached: impl Fn(&Track) -> bool,
        alive: impl Fn(&Track) -> bool,
    ) -> Option<&'a Track> {
        Self::preferred(self.find(key)?, cached, alive)
    }

    /// Pick the version of `entry` that should play, or `None` if none can.
    ///
    /// Priority, highest first:
    ///
    /// 1. **quality** — the best copy;
    /// 2. **downloaded** — a local file beats streaming at equal quality;
    /// 3. **availability** — a version that cannot play is never chosen.
    ///
    /// Availability is a filter rather than a tiebreak: a lossless copy whose
    /// URL has expired must not win, so it is skipped and the next-best live
    /// version is returned ("the preferred copy died, so we fell back"). When
    /// every version is dead the result is `None`, which is the design's
    /// "全部失效" state — the caller must not guess.
    ///
    /// `cached` and `alive` are callbacks because neither fact lives at this
    /// layer: one is the audio cache, the other the network. Quality has no
    /// per-stream value on a [`Track`] either, so it is taken from what the
    /// source is able to serve (see `quality_rank`).
    pub fn preferred(
        entry: &LibraryEntry,
        cached: impl Fn(&Track) -> bool,
        alive: impl Fn(&Track) -> bool,
    ) -> Option<&Track> {
        // A document saved before `versions` existed, or hand-edited to drop it,
        // still has the preferred track.
        let candidates: Vec<&Track> = if entry.versions.is_empty() {
            vec![&entry.track]
        } else {
            entry.versions.iter().collect()
        };

        // A pinned version wins outright, as long as it is still there. A pin the
        // platform has since removed must not make the song unplayable, so it
        // falls back to the ranking below.
        if let Some(pinned) = &entry.pinned {
            if let Some(track) = candidates
                .iter()
                .find(|track| track.key() == *pinned && alive(track))
            {
                return Some(*track);
            }
        }

        let mut best: Option<(&Track, (u8, bool))> = None;
        for track in candidates {
            if !alive(track) {
                continue;
            }
            let rank = (quality_rank(track), cached(track));
            if best.is_none_or(|(_, best_rank)| rank > best_rank) {
                best = Some((track, rank));
            }
        }
        best.map(|(track, _)| track)
    }

    fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_json::from_str::<LocalLibrary>(&text) {
            Ok(library) => library,
            Err(err) => {
                eprintln!(
                    "library at {} is unreadable ({err}); using an empty one",
                    path.display()
                );
                Self::default()
            }
        }
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        let text = serde_json::to_vec_pretty(self)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        platform::write_private(path, &text)
    }
}

fn entry_matches(entry: &LibraryEntry, key: &str) -> bool {
    LocalLibrary::key_of(&entry.track) == key
        || entry
            .versions
            .iter()
            .any(|v| LocalLibrary::key_of(v) == key)
}

/// Lowercased and whitespace-collapsed, so case and spacing never fork a key.
fn normalize_for_key(text: &str) -> String {
    util::collapse_whitespace(&text.to_lowercase())
}

/// Five-second buckets: the same recording reported as 269 s and 270 s must
/// share a key, while a three-minute song and a six-minute one must not.
fn duration_bucket(seconds: u64) -> u64 {
    if seconds == 0 {
        // Unknown, and kept separate from a real zero-length value.
        0
    } else {
        (seconds + 2) / 5
    }
}

/// The static quality tier of a version.
///
/// A [`Track`] carries no bitrate, so the honest statement this layer can make
/// is what the source is capable of: Bilibili can hand out lossless FLAC,
/// YouTube tops out at AAC 130K. 0 is the low tier, 1 the lossless-capable one.
fn quality_rank(track: &Track) -> u8 {
    u8::from(track.source.capabilities().lossless)
}

/// The current moment, as seconds since the epoch.
///
/// Exposed so the interface can compute an age ("3 分钟前") from a stored stamp
/// without needing a timezone at all.
pub fn now_seconds() -> u64 {
    unix_seconds()
}

/// Today's date and the day before it, as `2026-08-14`, in the machine's own
/// timezone.
///
/// All 最近播放's 今天 / 昨天 / 更早 grouping needs of a calendar. Only the first
/// two groups compare against a real date; everything older is one group, so this
/// is the whole date API the interface requires.
pub fn today_and_yesterday() -> (String, String) {
    let now = unix_seconds();
    (local_date(now), local_date(now.saturating_sub(86_400)))
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// The machine's offset from UTC, or zero when it cannot be asked.
///
/// Zero is the old behaviour, and it is a fallback rather than a guess: a wrong
/// offset would move dates by hours, which is the very thing this exists to stop.
fn local_offset() -> i64 {
    crate::platform::local_utc_offset().unwrap_or(0)
}

/// The civil date an instant falls on, in the machine's own timezone.
///
/// Not UTC, and that is the point: a play made at 01:00 in UTC+8 is still on the
/// *previous* UTC day, so 最近播放 filed it under 昨天 — for the first eight hours
/// of every day. The offset is read per call, which is cheap and follows daylight
/// saving.
fn local_date(seconds: u64) -> String {
    local_date_at(seconds, local_offset())
}

/// The same, with the offset spelled out, so it can be tested.
fn local_date_at(seconds: u64, offset: i64) -> String {
    date_string(shift(seconds, offset))
}

fn local_timestamp(seconds: u64) -> String {
    timestamp_string(shift(seconds, local_offset()))
}

/// Moves an instant by an offset in seconds, stopping at the epoch.
fn shift(seconds: u64, offset: i64) -> u64 {
    (seconds as i64 + offset).max(0) as u64
}

/// Today's date, in the machine's own timezone.
fn today() -> String {
    local_date(unix_seconds())
}

/// The civil date of an instant in UTC.
///
/// Kept separate from [`local_date`] on purpose: this is the pure function the
/// epoch tests pin down, and the timezone is applied on top of it.
fn date_string(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn timestamp_string(seconds: u64) -> String {
    let into_day = seconds % 86_400;
    let hour = into_day / 3_600;
    let minute = (into_day % 3_600) / 60;
    format!("{} {hour:02}:{minute:02}", date_string(seconds))
}

/// Days since the Unix epoch to a civil date (year, month, day).
///
/// The classic `civil_from_days` algorithm, so no date crate is needed for what
/// is only ever a display string.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A scratch path per test name, so parallel tests never share a file.
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "listenbli-library-{}-{name}.json",
            std::process::id()
        ))
    }

    fn track(
        bvid: &str,
        source: Source,
        title: &str,
        artist: Option<&str>,
        duration: u64,
    ) -> Track {
        Track {
            bvid: bvid.to_owned(),
            source,
            aid: 0,
            cid: 0,
            title: title.to_owned(),
            author: "上传者".to_owned(),
            duration,
            cover: None,
            album: None,
            artist: artist.map(str::to_owned),
        }
    }

    fn jingtian_bili() -> Track {
        track(
            "BV1jingtian",
            Source::Bilibili,
            "【4K修复】周杰伦 - 晴天",
            Some("周杰伦"),
            269,
        )
    }

    fn jingtian_youtube() -> Track {
        track(
            "dQw4w9WgXcQ",
            Source::Youtube,
            "周杰伦 - 晴天 (Official MV)",
            Some("周杰伦"),
            270,
        )
    }

    #[test]
    fn a_missing_file_loads_as_an_empty_library() {
        let path = scratch("missing");
        let _ = std::fs::remove_file(&path);
        let library = LocalLibrary::load_from(&path);
        assert_eq!(library.version, LIBRARY_VERSION);
        assert!(library.entries.is_empty());
        assert!(library.playlists.is_empty());
        assert!(library.recent.is_empty());
    }

    #[test]
    fn a_corrupt_file_loads_as_an_empty_library_instead_of_panicking() {
        let path = scratch("corrupt");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let library = LocalLibrary::load_from(&path);
        assert!(library.entries.is_empty());

        // A partially written document with fields missing must still load:
        // `version` alone is a valid (empty) library.
        std::fs::write(&path, br#"{"version":1}"#).unwrap();
        let library = LocalLibrary::load_from(&path);
        assert_eq!(library.version, 1);
        assert!(library.entries.is_empty() && library.playlists.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_saved_library_round_trips_through_the_file() {
        let path = scratch("round-trip");
        let _ = std::fs::remove_file(&path);

        let mut library = LocalLibrary::default();
        assert!(library.add(jingtian_bili()));
        library.playlists.push(Playlist::local("p1", "我的歌单"));
        library.playlists[0]
            .entries
            .push(LocalLibrary::key_of(&jingtian_bili()));
        library.record_play(&LocalLibrary::key_of(&jingtian_bili()));
        library.save_to(&path).unwrap();

        let back = LocalLibrary::load_from(&path);
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].track.title, jingtian_bili().title);
        assert_eq!(back.playlists.len(), 1);
        assert_eq!(back.playlists[0].title, "我的歌单");
        assert_eq!(back.playlists[0].entries.len(), 1);
        assert_eq!(back.recent.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    /// The key is what makes a Bilibili copy and a YouTube copy one library row,
    /// so it must not vary by platform, uploader or a second of duration.
    #[test]
    fn key_of_is_the_same_for_one_song_on_two_platforms() {
        let bili = jingtian_bili();
        let mut yt = jingtian_youtube();
        assert_eq!(LocalLibrary::key_of(&bili), LocalLibrary::key_of(&yt));

        // The uploader is not part of the identity: re-upload accounts differ.
        yt.author = "某个搬运号".to_owned();
        assert_eq!(LocalLibrary::key_of(&bili), LocalLibrary::key_of(&yt));

        // Neither is the platform id or the decoration in the title.
        yt.bvid = "another-id".to_owned();
        assert_eq!(LocalLibrary::key_of(&bili), LocalLibrary::key_of(&yt));
    }

    #[test]
    fn key_of_separates_different_songs() {
        let jingtian = jingtian_bili();
        let daoxiang = track("BV1daoxiang", Source::Bilibili, "稻香", Some("周杰伦"), 223);
        assert_ne!(
            LocalLibrary::key_of(&jingtian),
            LocalLibrary::key_of(&daoxiang)
        );

        // Same title, different real artist: different songs.
        let cover = track("BV1cover", Source::Youtube, "晴天", Some("别人"), 269);
        assert_ne!(
            LocalLibrary::key_of(&jingtian),
            LocalLibrary::key_of(&cover)
        );
    }

    #[test]
    fn adding_the_same_song_twice_merges_instead_of_duplicating() {
        let mut library = LocalLibrary::default();
        assert!(
            library.add(jingtian_bili()),
            "the first copy is a new entry"
        );
        assert!(
            !library.add(jingtian_youtube()),
            "the second platform copy is the same song"
        );

        assert_eq!(library.entries.len(), 1);
        let entry = &library.entries[0];
        assert_eq!(entry.versions.len(), 2, "both platform copies are kept");

        // Adding the very same platform copy again changes nothing.
        assert!(!library.add(jingtian_youtube()));
        assert_eq!(library.entries[0].versions.len(), 2);
    }

    #[test]
    fn adding_different_songs_creates_separate_entries() {
        let mut library = LocalLibrary::default();
        assert!(library.add(jingtian_bili()));
        assert!(library.add(track(
            "BV1daoxiang",
            Source::Bilibili,
            "稻香",
            Some("周杰伦"),
            223
        )));

        assert_eq!(library.entries.len(), 2);
        assert!(library
            .find(&LocalLibrary::key_of(&jingtian_bili()))
            .is_some());
        assert!(library.find("no such key").is_none());
    }

    #[test]
    fn find_mut_and_remove_follow_the_key() {
        let mut library = LocalLibrary::default();
        library.add(jingtian_bili());
        let key = LocalLibrary::key_of(&jingtian_bili());

        library
            .find_mut(&key)
            .expect("the entry exists")
            .playlists
            .push("p1".to_owned());
        assert_eq!(library.find(&key).unwrap().playlists, ["p1"]);

        assert!(library.remove(&key));
        assert!(library.find(&key).is_none());
        assert!(!library.remove(&key), "removing twice does nothing");
    }

    /// Removing a song must not leave a playlist or the recent list pointing at
    /// a row that can no longer resolve.
    #[test]
    fn removing_a_song_also_removes_it_from_playlists_and_recent() {
        let mut library = LocalLibrary::default();
        library.add(jingtian_bili());
        let key = LocalLibrary::key_of(&jingtian_bili());
        library.playlists.push(Playlist::local("p1", "我的歌单"));
        library.playlists[0].entries.push(key.clone());
        library.record_play(&key);

        assert!(library.remove(&key));
        assert!(library.playlists[0].entries.is_empty());
        assert!(library.recent.is_empty());
    }

    /// Quality first, then downloaded, then availability — and a dead copy is
    /// never chosen even when it is the best one.
    #[test]
    fn preferred_ranks_quality_then_cache_then_falls_back_when_the_best_is_dead() {
        let mut library = LocalLibrary::default();
        library.add(jingtian_bili()); // lossless-capable source
        library.add(jingtian_youtube()); // AAC-only source
        let entry = &library.entries[0];
        let bili = entry.versions[0].clone();
        let yt = entry.versions[1].clone();

        let always = |_: &Track| true;
        let never = |_: &Track| false;

        // Nothing cached: the higher-quality source wins.
        let picked = LocalLibrary::preferred(entry, never, always).unwrap();
        assert_eq!(picked.source, Source::Bilibili);
        assert_eq!(picked.key(), bili.key());

        // Quality outranks being downloaded: a lossless online copy beats a
        // cached AAC one.
        let cached_yt = |t: &Track| t.source == Source::Youtube;
        let picked = LocalLibrary::preferred(entry, cached_yt, always).unwrap();
        assert_eq!(picked.source, Source::Bilibili);

        // The high-quality copy died: fall back to the live one.
        let only_youtube_alive = |t: &Track| t.source == Source::Youtube;
        let picked = LocalLibrary::preferred(entry, never, only_youtube_alive).unwrap();
        assert_eq!(picked.source, Source::Youtube);
        assert_eq!(picked.key(), yt.key());
    }

    #[test]
    fn preferred_returns_none_when_every_version_is_dead() {
        let mut library = LocalLibrary::default();
        library.add(jingtian_bili());
        library.add(jingtian_youtube());
        let entry = &library.entries[0];

        assert!(LocalLibrary::preferred(entry, |_| false, |_| false).is_none());

        // A single-version entry whose only copy is dead is also None.
        let mut solo = LocalLibrary::default();
        solo.add(jingtian_bili());
        assert!(LocalLibrary::preferred(&solo.entries[0], |_| true, |_| false).is_none());
    }

    /// `resolve` is the same decision reached through a key.
    #[test]
    fn resolve_finds_the_preferred_version_by_key() {
        let mut library = LocalLibrary::default();
        library.add(jingtian_bili());
        library.add(jingtian_youtube());
        let key = LocalLibrary::key_of(&jingtian_bili());

        let picked = library.resolve(&key, |_| false, |_| true).unwrap();
        assert_eq!(picked.source, Source::Bilibili);
        assert!(library.resolve("missing", |_| true, |_| true).is_none());
        assert!(library.resolve(&key, |_| true, |_| false).is_none());
    }

    #[test]
    fn a_play_moves_to_the_top_and_is_never_duplicated() {
        let mut library = LocalLibrary::default();
        library.record_play("a");
        library.record_play("b");
        assert_eq!(library.recent[0].key, "b");

        library.record_play("a");
        assert_eq!(library.recent[0].key, "a", "replaying promotes, not adds");
        assert_eq!(library.recent.len(), 2);
        assert!(library.recent[0].at.contains('-'), "a dated stamp");

        // An empty key is not a play.
        library.record_play("   ");
        assert_eq!(library.recent.len(), 2);
    }

    #[test]
    fn the_recent_list_is_capped_at_max_recent() {
        let mut library = LocalLibrary::default();
        for index in 0..MAX_RECENT + 20 {
            library.record_play(&format!("key-{index}"));
        }
        assert_eq!(library.recent.len(), MAX_RECENT);
        assert_eq!(library.recent[0].key, format!("key-{}", MAX_RECENT + 19));

        // Replaying an old song does not grow the list past the cap.
        library.record_play("key-0");
        assert_eq!(library.recent.len(), MAX_RECENT);
        assert_eq!(library.recent[0].key, "key-0");
    }

    /// The design's read/write declaration: a local playlist is editable, a
    /// platform collection never is.
    #[test]
    fn platform_playlists_are_read_only_and_local_ones_are_not() {
        let local = Playlist::local("p1", "我的歌单");
        assert_eq!(local.origin, PlaylistOrigin::Local);
        assert!(!local.readonly);
        assert!(local.is_writable());

        for source in [Source::Bilibili, Source::Youtube] {
            let synced = Playlist::platform("fav-1", "默认收藏夹", source);
            assert_eq!(synced.origin, PlaylistOrigin::Platform(source));
            assert!(synced.readonly, "a platform collection is never writable");
            assert!(!synced.is_writable());
        }
    }

    #[test]
    fn the_library_lives_beside_the_config_but_not_in_it() {
        let path = LocalLibrary::path();
        assert_eq!(path.file_name().unwrap(), "library.json");
        assert!(path.starts_with(platform::config_dir()));
        assert!(path.is_absolute());
    }

    /// The date helper exists only to avoid a dependency, so it is checked
    /// against the epochs it is known to be correct for.
    /// A pin is the listener's own answer to "which copy", and it outranks the
    /// quality/downloaded ranking — otherwise pressing 改用这一版 in 来源详情
    /// would be undone by the very rule the button exists to override.
    #[test]
    fn a_pinned_version_outranks_the_ranking() {
        let mut library = LocalLibrary::default();
        let mut bili = track("BV1", Source::Bilibili, "晴天", Some("周杰伦"), 269);
        bili.duration = 269;
        let yt = track("youtube1", Source::Youtube, "晴天", Some("周杰伦"), 269);
        library.add(bili.clone());
        library.add(yt.clone());
        let key = LocalLibrary::key_of(&bili);

        let always = |_: &Track| false;
        let alive = |_: &Track| true;
        let entry = library
            .find(&key)
            .expect("both copies merged into one song");

        // Bilibili is the lossless source, so it wins on merit.
        let best = LocalLibrary::preferred(entry, always, alive).expect("something plays");
        assert_eq!(best.source, Source::Bilibili);

        // Pinning YouTube makes it the answer, ranking or not.
        assert!(library.pin(&key, Some(yt.key())));
        let entry = library.find(&key).expect("still there");
        let best = LocalLibrary::preferred(entry, always, alive).expect("something plays");
        assert_eq!(best.source, Source::Youtube);

        // A pin for a version that is no longer playable must not silence the
        // song: it falls back to the ranking. The closure answers "is this copy
        // alive", so it is the *other* source that stays true.
        let youtube_is_gone = |track: &Track| track.source != Source::Youtube;
        let best = LocalLibrary::preferred(entry, always, youtube_is_gone).expect("still plays");
        assert_eq!(best.source, Source::Bilibili);

        // And a pin naming a version this entry does not hold is refused.
        assert!(!library.pin(&key, Some("nothing-here".to_owned())));
        assert!(!library.pin("no-such-song", Some(yt.key())));
        assert!(library.pin(&key, None), "clearing a pin always works");
        assert!(library.find(&key).expect("there").pinned.is_none());
    }

    /// The meters on 来源与同步 come from this diff, so it has to be a real one:
    /// what was there before against what the platform just returned.
    #[test]
    fn a_sync_reports_what_it_added_and_what_somebody_removed() {
        let mut library = LocalLibrary::default();
        let bili = track("BV1sync", Source::Bilibili, "晴天", Some("周杰伦"), 269);
        let youtube = track("yt-sync", Source::Youtube, "晴天", Some("周杰伦"), 269);
        library.add(bili.clone());
        library.add(youtube);
        let jing = LocalLibrary::key_of(&bili);
        let other = "七里香|周杰伦|60".to_owned();

        let first = library.sync_playlist(
            "platform:bilibili:1",
            "默认收藏夹",
            Source::Bilibili,
            vec![jing.clone(), other.clone()],
        );
        assert_eq!((first.added, first.removed), (2, 0), "everything is new");
        assert_eq!(first.merged, 1, "晴天 has a copy on each platform");

        // The listener un-favourites one and adds another on the platform's side.
        let second = library.sync_playlist(
            "platform:bilibili:1",
            "默认收藏夹",
            Source::Bilibili,
            vec![jing.clone(), "稻香|周杰伦|45".to_owned()],
        );
        assert_eq!((second.added, second.removed), (1, 1));
        assert!(second.at_unix > 0, "an age needs an instant");

        // Re-syncing the same list changes nothing, and says so.
        let third = library.sync_playlist(
            "platform:bilibili:1",
            "默认收藏夹",
            Source::Bilibili,
            vec![jing, "稻香|周杰伦|45".to_owned()],
        );
        assert_eq!((third.added, third.removed), (0, 0));

        // It replaces the collection rather than piling up copies of it.
        assert_eq!(
            library
                .playlists
                .iter()
                .filter(|playlist| playlist.id == "platform:bilibili:1")
                .count(),
            1
        );
        assert!(library.last_sync.is_some());
    }

    /// The bug this exists for: in UTC+8 a play made at 01:00 is still on the
    /// previous UTC day, so 最近播放 filed the first eight hours of every day
    /// under 昨天.
    #[test]
    fn a_date_stamp_follows_the_machine_timezone() {
        // 2024-02-29T16:00Z, which is 2024-03-01T00:00 in UTC+8.
        let late_utc = 1_709_164_800 + 16 * 3_600;
        assert_eq!(date_string(late_utc), "2024-02-29", "the UTC date");
        assert_eq!(
            local_date_at(late_utc, 8 * 3_600),
            "2024-03-01",
            "and the date where the machine actually is"
        );

        // The other direction, and the epoch edge: a negative offset on the epoch
        // itself saturates rather than wrapping round to 2554 — an instant cannot
        // be before the epoch.
        assert_eq!(local_date_at(1_709_164_800, -8 * 3_600), "2024-02-28");
        assert_eq!(local_date_at(0, 8 * 3_600), "1970-01-01");
        assert_eq!(shift(0, -8 * 3_600), 0);
        assert_eq!(local_date_at(0, -8 * 3_600), "1970-01-01");

        // UTC itself is still what it was.
        assert_eq!(local_date_at(1_709_164_800, 0), "2024-02-29");
    }

    /// What the local dates actually changed: a stamp used to be written in UTC,
    /// so a play made at 01:00 in UTC+8 showed 17:00 — the *clock* was wrong by
    /// the offset. (The grouping was consistent either way, because both sides of
    /// that comparison were UTC.)
    #[test]
    fn a_stamp_uses_the_machine_clock_not_utc() {
        if local_offset() == 0 {
            // A machine at UTC cannot tell the two apart, so there is nothing to
            // assert rather than a test that cannot fail.
            return;
        }
        let now = unix_seconds();
        assert_ne!(
            local_timestamp(now),
            timestamp_string(now),
            "a machine off UTC must stamp its own clock"
        );
    }

    /// The contract the dates rest on: whatever the platform reports has to be a
    /// real timezone, or the fallback would be better than the answer.
    #[test]
    fn a_machine_offset_is_a_plausible_timezone() {
        let Some(offset) = crate::platform::local_utc_offset() else {
            // Windows, and any machine that will not say: UTC is the documented
            // fallback rather than a bug.
            return;
        };
        assert!(
            offset.abs() <= 14 * 3_600,
            "no timezone is more than 14 hours from UTC: {offset}"
        );
        assert_eq!(offset % 60, 0, "offsets are whole minutes: {offset}");
    }

    #[test]
    fn civil_dates_match_the_known_epochs() {
        assert_eq!(date_string(0), "1970-01-01");
        // 2000-01-01 is 10 957 days later.
        assert_eq!(date_string(10_957 * 86_400), "2000-01-01");
        // A leap day.
        assert_eq!(date_string(1_709_164_800), "2024-02-29");
        assert_eq!(timestamp_string(0), "1970-01-01 00:00");
        assert_eq!(timestamp_string(86_399), "1970-01-01 23:59");
    }
}

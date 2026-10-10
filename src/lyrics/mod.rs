//! Lyrics: Bilibili's own subtitle track first, NetEase as a fallback, with a
//! small on-disk cache so repeated plays do not re-query the third party.

pub mod bilibili;
pub mod lrc;
pub mod lrclib;
pub mod netease;

use std::path::{Path, PathBuf};

use crate::api::client::Api;
use crate::api::models::{Source, Track};
use crate::platform;

pub use lrc::{LyricLine, Lyrics, LyricsSource};

/// Cache generation.
///
/// Bump this whenever the *policy* in `fetch_for` changes, so documents written
/// under the old rules are ignored rather than served forever. `v2` exists
/// because `v1` picked up machine subtitles that belonged to other videos.
const CACHE_GENERATION: &str = "v2";

/// How far into the video a *machine* subtitle has to reach to be believed.
///
/// Coverage is the last line's timestamp over the video's duration.
///
/// Set to half the video on purpose, and with the cost understood. Measured
/// against the 40 machine tracks this app had cached from real logged-in use,
/// coverage runs 0.000–18.5 with a clean gap between 0.098 and 0.292; a floor of
/// 0.2 sits inside that gap and drops only the plainly degenerate documents
/// (1–17 lines covering the opening seconds). Half the video discards two more
/// that are substantial but partial — a 43-line CC stopping at 87 s of a
/// 5-minute video, and 599 lines covering 1296 s of a 63-minute collection — so
/// 11 of those 40 documents go instead of 9.
///
/// The extra strictness does *not* buy protection against the wrong-file case
/// that prompted this guard: that document ends at 51.6% of its video, clearing
/// 0.5 by eight seconds. A floor cannot catch that class at any threshold — see
/// `machine_subtitle_reaches_into`.
const MIN_MACHINE_COVERAGE: f64 = 0.5;

/// Would a machine subtitle be plausible for a video of `duration_secs`?
///
/// Only ever consulted for a *machine* track: a human wrote that subtitle for
/// this video on purpose, and may well have covered only part of it.
///
/// `duration_secs == 0` means the length is not known yet, which is not evidence
/// against the subtitle, so it passes.
///
/// Note what this cannot do. A subtitle belonging to a completely different
/// video lands wherever *that* video's coverage lands, so it can sit anywhere in
/// the same band as a legitimately partial one — the case that prompted this
/// guard measured 0.516, which the current 0.5 floor lets through by eight
/// seconds. No local heuristic can tell "the uploader only subtitled part of
/// this" from "Bilibili served another video's file". That distinction is why
/// NetEase is ranked above every machine track; this floor only removes the ones
/// that stop implausibly early.
fn machine_subtitle_reaches_into(duration_secs: u64, lyrics: &Lyrics) -> bool {
    if duration_secs == 0 {
        return true;
    }
    let Some(last) = lyrics.lines.iter().map(|line| line.time).max() else {
        return false;
    };
    last.as_secs_f64() >= duration_secs as f64 * MIN_MACHINE_COVERAGE
}

/// Resolve lyrics for `track`.
///
/// Order: a human-written Bilibili CC track wins outright, then NetEase, and
/// only then Bilibili's *machine* (ASR) subtitle.
///
/// A machine subtitle used to outrank NetEase. It no longer does. An ASR
/// transcript of singing is not a lyric, and the track is not reliably this
/// video's: `/x/player/v2` has been observed returning one video's AI subtitle
/// for another video's `cid`. A human track has neither problem, so it keeps the
/// top slot. NetEase already gets the other direction right — an upload no
/// database knows about (talk, variety, live set) yields nothing there and falls
/// through to the machine subtitle, which is what those videos want.
///
/// A machine track that does get used still has to clear `MIN_MACHINE_COVERAGE`,
/// which throws out the obviously truncated ones. That floor is not what makes
/// this correct — see `machine_subtitle_reaches_into`.
///
/// Never returns an error: a missing lyric is a normal outcome, not a failure
/// worth interrupting playback for.
pub fn fetch_for(api: &Api, track: &Track, cache_root: &Path) -> Lyrics {
    if let Some(cached) = load_cached(cache_root, &track.key()) {
        return cached;
    }

    // Fetch the machine track now so it is on hand as a fallback, but do not
    // return it until NetEase has had its turn — and only keep it if it reaches
    // far enough into the video to be credible.
    let mut machine_fallback: Option<Lyrics> = None;
    if track.source == Source::Bilibili && track.cid != 0 {
        match bilibili::fetch(api, &track.bvid, track.cid) {
            Ok(Some(subtitle)) if subtitle.machine_generated => {
                if machine_subtitle_reaches_into(track.duration, &subtitle.lyrics) {
                    machine_fallback = Some(subtitle.lyrics);
                } else {
                    eprintln!(
                        "bilibili machine subtitle for {} stops far short of the \
                         video's {}s; ignoring it",
                        track.bvid, track.duration
                    );
                }
            }
            Ok(Some(subtitle)) => return finish(cache_root, &track.key(), subtitle.lyrics),
            Ok(None) => {}
            Err(err) => eprintln!("bilibili subtitle for {} failed: {err}", track.bvid),
        }
    }

    // LRCLib goes first for YouTube: it is the provider built around music, and
    // what YouTube holds is mostly music videos. For a Bilibili upload it is tried
    // after NetEase, because NetEase's catalogue covers Chinese pop better and its
    // translations are merged into the document.
    if track.source == Source::Youtube {
        if let Some(lyrics) = try_lrclib(api, track) {
            return finish(cache_root, &track.key(), lyrics);
        }
    }

    let netease = netease::fetch(api, track);
    if let Err(err) = &netease {
        eprintln!("netease lyrics for {} failed: {err}", track.bvid);
    }
    match netease {
        Ok(Some(lyrics)) => return finish(cache_root, &track.key(), lyrics),
        Ok(None) | Err(_) => {}
    }

    if track.source == Source::Bilibili {
        if let Some(lyrics) = try_lrclib(api, track) {
            return finish(cache_root, &track.key(), lyrics);
        }
    }

    match machine_fallback {
        Some(lyrics) => finish(cache_root, &track.key(), lyrics),
        None => Lyrics::empty(LyricsSource::None),
    }
}

/// LRCLib, with its failures logged rather than propagated.
///
/// It is one source among several: a network hiccup here must not stop the chain,
/// and a miss is the normal case for an obscure track.
fn try_lrclib(api: &Api, track: &Track) -> Option<Lyrics> {
    match lrclib::fetch(api, track) {
        Ok(Some(lyrics)) => Some(lyrics),
        Ok(None) => None,
        Err(err) => {
            eprintln!("lrclib lyrics for {} failed: {err}", track.key());
            None
        }
    }
}

/// Cache a resolved document and hand it back.
///
/// `key` is [`Track::key`](crate::api::models::Track::key), not the bare `bvid`:
/// a YouTube id and a Bilibili id must not be able to share a document.
fn finish(cache_root: &Path, key: &str, lyrics: Lyrics) -> Lyrics {
    store_cached(cache_root, key, &lyrics);
    lyrics
}

fn cache_file(cache_root: &Path, bvid: &str) -> Option<PathBuf> {
    let safe: String = bvid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        return None;
    }
    Some(
        cache_root
            .join("lyrics")
            .join(CACHE_GENERATION)
            .join(format!("{safe}.json")),
    )
}

pub fn load_cached(cache_root: &Path, bvid: &str) -> Option<Lyrics> {
    let path = cache_file(cache_root, bvid)?;
    let text = std::fs::read_to_string(path).ok()?;
    parse_cached(&text)
}

/// Decode one cached lyric document.
///
/// The cache stores *parsed* lines, so entries written before credits were
/// filtered out still carry them; normalizing on the way in keeps every old
/// cache file clean without a format bump.
fn parse_cached(text: &str) -> Option<Lyrics> {
    let mut lyrics: Lyrics = serde_json::from_str(text).ok()?;
    lyrics.lines.retain(|line| !lrc::is_credit_line(&line.text));
    Some(lyrics)
}

pub fn store_cached(cache_root: &Path, bvid: &str, lyrics: &Lyrics) {
    let Some(path) = cache_file(cache_root, bvid) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = platform::ensure_dir(parent);
    }
    // A failure here only costs a re-query next run.
    if let Ok(text) = serde_json::to_vec(lyrics) {
        let _ = std::fs::write(&path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_rejects_path_traversal() {
        assert!(
            cache_file(&root(), "../../etc/passwd").is_none()
                || !cache_file(&root(), "../../etc/passwd")
                    .unwrap()
                    .to_string_lossy()
                    .contains("..")
        );
        assert!(cache_file(&root(), "").is_none());
        assert!(cache_file(&root(), "BV1xx411c7mD").is_some());
    }

    /// The cache root the tests write under. Also proves the path is built from
    /// the argument and not from the platform directory.
    fn root() -> PathBuf {
        std::env::temp_dir().join("listenbli-lyrics-root-tests")
    }

    #[test]
    fn the_cache_root_argument_decides_where_documents_live() {
        let elsewhere = PathBuf::from("/Volumes/Big/listenbli");
        let path = cache_file(&elsewhere, "BV1xx411c7mD").expect("a valid bvid yields a path");
        assert!(
            path.starts_with(elsewhere.join("lyrics").join(CACHE_GENERATION)),
            "got {path:?}"
        );
        assert_ne!(
            path,
            cache_file(&root(), "BV1xx411c7mD").unwrap(),
            "a different root must be a different document"
        );
    }

    /// The stored document is found under the root it was written to, and not
    /// under another one — this is the cache-hit path `fetch_for` tries first.
    #[test]
    fn a_document_is_only_visible_under_the_root_it_was_stored_in() {
        let root = std::env::temp_dir().join(format!("listenbli-lyrics-{}", std::process::id()));
        let other = root.join("elsewhere");
        let _ = std::fs::remove_dir_all(&root);

        let lyrics = lyrics_at(&[1, 2, 3]);
        store_cached(&root, "BV1xx411c7mD", &lyrics);
        assert!(cache_file(&root, "BV1xx411c7mD").unwrap().is_file());

        let loaded = load_cached(&root, "BV1xx411c7mD").expect("stored under this root");
        assert_eq!(loaded.lines.len(), 3);
        assert!(
            load_cached(&other, "BV1xx411c7mD").is_none(),
            "another root must not see it"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    fn lyrics_at(seconds: &[u64]) -> Lyrics {
        Lyrics {
            lines: seconds
                .iter()
                .map(|s| LyricLine {
                    time: std::time::Duration::from_secs(*s),
                    text: "line".into(),
                    translation: None,
                })
                .collect(),
            source: LyricsSource::BilibiliSubtitle,
        }
    }

    #[test]
    fn a_machine_subtitle_stopping_in_the_opening_seconds_is_dropped() {
        // The shapes the census turned up: a single line, and a handful of lines
        // inside the first seconds of a long video.
        assert!(!machine_subtitle_reaches_into(304, &lyrics_at(&[0])));
        assert!(!machine_subtitle_reaches_into(
            681,
            &lyrics_at(&[0, 1, 2, 3])
        ));
        assert!(!machine_subtitle_reaches_into(
            1150,
            &lyrics_at(&[7, 20, 39])
        ));
        assert!(!machine_subtitle_reaches_into(
            494,
            &lyrics_at(&[0, 50, 98])
        ));
    }

    #[test]
    fn a_machine_subtitle_covering_real_ground_is_kept() {
        assert!(machine_subtitle_reaches_into(290, &lyrics_at(&[0, 220])));
        assert!(machine_subtitle_reaches_into(269, &lyrics_at(&[14, 250])));
        assert!(machine_subtitle_reaches_into(494, &lyrics_at(&[21, 400])));
        // Exactly half is enough — the floor is inclusive.
        assert!(machine_subtitle_reaches_into(300, &lyrics_at(&[0, 150])));
    }

    /// The price of the 0.5 floor, pinned so it stays a decision rather than a
    /// surprise. Both of these clear a 0.2 floor and are plausibly just partial.
    #[test]
    fn the_half_video_floor_also_drops_substantial_partial_tracks() {
        // 43 lines stopping at 87 s of a 5-minute video (coverage 0.29).
        assert!(!machine_subtitle_reaches_into(298, &lyrics_at(&[0, 86])));
        // 599 lines covering 1296 s of a 63-minute collection (coverage 0.34).
        assert!(!machine_subtitle_reaches_into(3803, &lyrics_at(&[0, 1296])));
    }

    #[test]
    fn an_unknown_duration_never_rejects_a_subtitle() {
        // The length is only known once `playurl` has answered; until then there
        // is no evidence against the subtitle.
        assert!(machine_subtitle_reaches_into(0, &lyrics_at(&[0])));
    }

    #[test]
    fn a_subtitle_with_no_lines_is_rejected() {
        assert!(!machine_subtitle_reaches_into(300, &lyrics_at(&[])));
    }

    /// Documents the limit on purpose, so nobody later assumes the floor covers
    /// this class of bug and removes the NetEase ordering that actually does.
    #[test]
    fn the_floor_still_lets_the_wrong_file_through() {
        // The document that prompted the guard: 63 lines of another video's
        // transcript, ending at 254.8 s of a 494 s video. That is a coverage of
        // 0.516, so the 0.5 floor passes it by eight seconds. It is the NetEase
        // ordering, not this floor, that keeps it off the screen.
        let wrong = Lyrics {
            lines: vec![
                LyricLine {
                    time: std::time::Duration::from_secs_f64(21.0),
                    text: "自从我加入WB以来".into(),
                    translation: None,
                },
                LyricLine {
                    time: std::time::Duration::from_secs_f64(254.8),
                    text: "让LPL的欢呼声响彻伦敦".into(),
                    translation: None,
                },
            ],
            source: LyricsSource::BilibiliSubtitle,
        };
        assert!(
            machine_subtitle_reaches_into(494, &wrong),
            "0.516 > {MIN_MACHINE_COVERAGE}; the ordering is what fixes this case"
        );
        // The same file stored under the 43-minute video it was also served for
        // is a coverage of 0.098, and the floor does catch it there.
        assert!(!machine_subtitle_reaches_into(2590, &wrong));
    }

    #[test]
    fn cache_lives_under_the_current_generation() {
        // A policy change must not keep serving documents written under the old
        // rules, so the generation is part of the path and bumping it orphans
        // every earlier file.
        let path = cache_file(&root(), "BV1xx411c7mD").expect("a valid bvid yields a path");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert!(parts.iter().any(|p| p == "lyrics"), "got {path:?}");
        assert!(
            parts.iter().any(|p| p == CACHE_GENERATION),
            "cache must be scoped to {CACHE_GENERATION}: {path:?}"
        );
    }

    #[test]
    fn credit_lines_are_dropped_while_parsing() {
        // The shape NetEase actually serves: credits carry timestamps too.
        let lrc = "[00:00.00]作词 : Chieh-lun Chou\n\
                   [00:02.25]词：周杰伦\n\
                   [00:04.50]曲：周杰伦\n\
                   [00:06.75]编曲：周杰伦\n\
                   [00:09.00]制作人：周杰伦\n\
                   [00:10.00]Producer: Someone\n\
                   [00:18.68]故事的小黄花\n";
        let lines = lrc::parse_lrc(lrc);
        assert_eq!(
            lines.len(),
            1,
            "only the real lyric should survive: {lines:?}"
        );
        assert_eq!(lines[0].1, "故事的小黄花");
    }

    #[test]
    fn a_lyric_that_merely_mentions_songwriting_is_kept() {
        let lines = lrc::parse_lrc(
            "[00:01.00]他作了曲，我写了词\n[00:02.00]歌剧：一夜\n[00:03.00]曲终人散",
        );
        assert_eq!(lines.len(), 3, "these are lyrics, not credits: {lines:?}");
    }

    #[test]
    fn a_credits_only_document_has_no_lyrics() {
        assert!(lrc::parse_lrc("[00:00.00]作词 : 某人\n[00:01.00]作曲 : 某人").is_empty());
    }

    #[test]
    fn old_cache_entries_lose_their_credit_lines() {
        let cached = Lyrics {
            lines: vec![
                LyricLine {
                    time: std::time::Duration::ZERO,
                    text: "作词 : 陈涛".into(),
                    translation: None,
                },
                LyricLine {
                    time: std::time::Duration::from_secs(1),
                    text: "作曲 : 王晓锋".into(),
                    translation: None,
                },
                LyricLine {
                    time: std::time::Duration::from_secs(10),
                    text: "昨天所有的荣誉，已变成遥远的回忆。".into(),
                    translation: Some("all the glory of yesterday".into()),
                },
            ],
            source: LyricsSource::Netease,
        };
        let text = serde_json::to_string(&cached).unwrap();

        let decoded = parse_cached(&text).expect("the cache should decode");
        assert_eq!(decoded.lines.len(), 1);
        assert_eq!(decoded.lines[0].text, "昨天所有的荣誉，已变成遥远的回忆。");
        assert_eq!(
            decoded.lines[0].translation.as_deref(),
            Some("all the glory of yesterday")
        );
    }

    #[test]
    fn lyrics_round_trip_through_json() {
        let lyrics = Lyrics {
            lines: vec![
                LyricLine {
                    time: std::time::Duration::from_millis(18_684),
                    text: "hello".into(),
                    translation: Some("你好".into()),
                },
                LyricLine {
                    time: std::time::Duration::from_secs(30),
                    text: "world".into(),
                    translation: None,
                },
            ],
            source: LyricsSource::Netease,
        };
        let text = serde_json::to_string(&lyrics).unwrap();
        let back: Lyrics = serde_json::from_str(&text).unwrap();
        assert_eq!(back.lines.len(), 2);
        assert_eq!(back.lines[0].time, std::time::Duration::from_millis(18_684));
        assert_eq!(back.lines[0].translation.as_deref(), Some("你好"));
        assert_eq!(back.source, LyricsSource::Netease);
    }
}

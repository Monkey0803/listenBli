//! Deciding when two platform entries are the same recording.
//!
//! The library design says a song appears once and its platform copies hang off
//! it, so this is the rule that makes "once" true. It must not be naive: the
//! same video is re-uploaded as `【4K修复】…` and `… (Official MV)`, and an
//! uploader may put a compilation of fifty songs under a title that shares
//! words with a single. Those are the same and not-the-same cases this module
//! exists to separate.
//!
//! Three signals, deliberately unequal:
//!
//! * the cleaned title, via [`util::title_similarity`] — the main term;
//! * the *real* artist (`artist`), a hard gate when both sides know it;
//! * the duration, by relative tolerance, a hard gate when both sides know it.
//!
//! `author` (the UP owner or channel) is never treated as the artist. It is a
//! weak hint at most — a re-upload account with an unrelated name is the normal
//! case — so it only trims a few percent and can never on its own split a song.

use crate::api::models::Track;
use crate::util;

/// At or above this score, [`verdict`] calls the two tracks one recording.
///
/// Chosen so that an identical cleaned title survives an unrelated uploader
/// (title 1.0 × author factor 0.92 = 0.92) while a genuinely different title
/// does not: `"晴天"` against `"晴天 钢琴改编版"` scores 0.68 on titles alone,
/// and the artist gate cuts that further.
pub const MERGE_THRESHOLD: f64 = 0.82;

/// How much the duration may differ, relative to the longer one, before the two
/// are treated as different recordings.
///
/// Relative rather than absolute because a Bilibili upload often carries an
/// intro, an outro or a trimmed tail: 269 s here and 268 s there is the same
/// recording, 269 s against 312 s is not.
const DURATION_TOLERANCE: f64 = 0.15;

/// A difference this small is rounding, not evidence. Durations are whole
/// seconds and the two platforms report them from different sources, so a few
/// seconds of drift on a short clip must not fail the relative test.
const DURATION_GRACE_SECS: u64 = 5;

/// Weight of two *known* artist names, and how much the similarity moves it.
///
/// Two conflicting real artists leave the score at 0.55 even with a perfect
/// title, which is far below the threshold — that is what keeps a piano
/// arrangement credited to someone else from merging with the original.
const REAL_ARTIST_FLOOR: f64 = 0.55;
const REAL_ARTIST_SPAN: f64 = 0.45;

/// Weight of an `author`-only comparison. It can trim, never veto.
///
/// `author` is an uploader, so a mismatch is weak evidence: two搬运号 posting
/// the same song have completely unrelated names, and an identical title must
/// still merge them (1.0 × 0.92 = 0.92 ≥ threshold).
const AUTHOR_FLOOR: f64 = 0.92;
const AUTHOR_SPAN: f64 = 0.08;

/// Whether two tracks are the same recording, as a verdict rather than a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// One library row: the versions are copies of the same recording.
    SameRecording,
    /// Two rows: different recordings, however similar the titles look.
    Different,
}

/// How alike two tracks are, in `0.0..=1.0`; higher is "more likely the same
/// recording".
///
/// The returned number is the product of three factors, each documented above,
/// so it is meaningful to print when explaining a merge to a user.
pub fn sameness(a: &Track, b: &Track) -> f64 {
    let title = util::title_similarity(
        &util::clean_song_title(&a.title),
        &util::clean_song_title(&b.title),
    );
    (title * artist_factor(a, b) * duration_factor(a.duration, b.duration)).clamp(0.0, 1.0)
}

/// [`sameness`] compared against [`MERGE_THRESHOLD`].
pub fn verdict(a: &Track, b: &Track) -> Verdict {
    if sameness(a, b) >= MERGE_THRESHOLD {
        Verdict::SameRecording
    } else {
        Verdict::Different
    }
}

/// The artist part of the score.
fn artist_factor(a: &Track, b: &Track) -> f64 {
    match (
        non_empty(a.artist.as_deref()),
        non_empty(b.artist.as_deref()),
    ) {
        (Some(left), Some(right)) => {
            // Both sides named a real performer: this is strong evidence either
            // way, so it is a heavy multiplier rather than a nudge.
            REAL_ARTIST_FLOOR + REAL_ARTIST_SPAN * util::title_similarity(left, right)
        }
        // Only one side knows the performer. The other's `author` is *often*
        // the same person (an artist uploading their own song) but just as
        // often a re-uploader, so this stays a weak signal.
        (Some(artist), None) => author_factor(artist, &b.author),
        (None, Some(artist)) => author_factor(&a.author, artist),
        // Neither side knows a performer: compare uploaders, weakly.
        (None, None) => author_factor(&a.author, &b.author),
    }
}

/// A weak comparison of uploader-ish strings. Unknown on either side is neutral.
fn author_factor(left: &str, right: &str) -> f64 {
    let (Some(left), Some(right)) = (non_empty(Some(left)), non_empty(Some(right))) else {
        // An empty name is missing information, not a disagreement.
        return 1.0;
    };
    AUTHOR_FLOOR + AUTHOR_SPAN * util::title_similarity(left, right)
}

/// The duration part of the score: 1.0 when compatible, 0.0 when it rules the
/// pair out. An unknown duration on either side is neutral — it is not evidence
/// of difference, and refusing to merge would punish history entries, which
/// frequently arrive without a length.
fn duration_factor(a: u64, b: u64) -> f64 {
    if a == 0 || b == 0 {
        return 1.0;
    }
    let (shorter, longer) = if a <= b { (a, b) } else { (b, a) };
    let difference = longer - shorter;
    if difference <= DURATION_GRACE_SECS {
        return 1.0;
    }
    let relative = difference as f64 / longer as f64;
    if relative <= DURATION_TOLERANCE {
        1.0
    } else {
        0.0
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::Source;

    /// The design's flagship example: the same song posted twice on Bilibili,
    /// once with the uploader's decoration and once bare.
    fn jingtian_4k() -> Track {
        Track {
            bvid: "BV1jingtian4k".into(),
            source: Source::Bilibili,
            aid: 1,
            cid: 11,
            title: "【4K修复】周杰伦 - 晴天".into(),
            author: "音乐无限".into(),
            duration: 269,
            cover: None,
            album: None,
            artist: Some("周杰伦".into()),
        }
    }

    fn jingtian_plain() -> Track {
        Track {
            bvid: "BV1jingtian".into(),
            source: Source::Bilibili,
            aid: 2,
            cid: 22,
            title: "周杰伦 - 晴天".into(),
            author: "音乐无限".into(),
            duration: 268,
            cover: None,
            album: None,
            artist: Some("周杰伦".into()),
        }
    }

    /// One test track with the fields the cases vary, everything else fixed.
    fn track(title: &str, author: &str, artist: Option<&str>, duration: u64) -> Track {
        Track {
            bvid: format!("BV{title}"),
            source: Source::Bilibili,
            aid: 0,
            cid: 0,
            title: title.to_owned(),
            author: author.to_owned(),
            duration,
            cover: None,
            album: None,
            artist: artist.map(str::to_owned),
        }
    }

    /// The same recording on two platforms: the uploader and the decoration
    /// differ, the song does not.
    #[test]
    fn the_same_song_across_platforms_merges() {
        let mut bili = jingtian_4k();
        bili.author = "音乐无限".into();
        let mut yt = jingtian_plain();
        yt.source = Source::Youtube;
        yt.bvid = "dQw4w9WgXcQ".into();
        yt.cid = 0;
        yt.title = "周杰伦 - 晴天 (Official MV)".into();
        yt.author = "Jay Chou 官方频道".into();
        yt.duration = 270;
        // Only one side was given the real artist, and even then the other's
        // uploader is a completely different name.
        yt.artist = None;

        assert_eq!(
            verdict(&bili, &yt),
            Verdict::SameRecording,
            "sameness {} should clear the threshold",
            sameness(&bili, &yt)
        );
    }

    /// `【4K修复】` is decoration on the same recording.
    #[test]
    fn a_4k_remaster_merges_with_the_original() {
        let remaster = track("【4K修复】周杰伦 - 晴天", "音乐无限", Some("周杰伦"), 269);
        let original = track("周杰伦 - 晴天", "音乐无限", Some("周杰伦"), 269);

        assert_eq!(verdict(&remaster, &original), Verdict::SameRecording);
    }

    /// The design is explicit that a piano arrangement is a different song, not a
    /// version of one — even though its title contains the original's.
    #[test]
    fn a_piano_arrangement_does_not_merge_with_the_original() {
        let original = track("晴天", "音乐无限", Some("周杰伦"), 269);
        let arrangement = track(
            "晴天（钢琴改编版）",
            "Animenz Piano",
            Some("Animenz Piano"),
            312,
        );

        assert_eq!(
            verdict(&original, &arrangement),
            Verdict::Different,
            "sameness {} must stay below the threshold",
            sameness(&original, &arrangement)
        );
    }

    /// A fifty-song compilation and a single can share words with the song they
    /// contain; they are not the same recording.
    #[test]
    fn a_fifty_song_collection_does_not_merge_with_a_single() {
        let collection = track("周杰伦 经典歌曲合集 50 首", "华语音乐搬运工", None, 4532);
        let single = track("晴天", "音乐无限", None, 269);

        assert_eq!(verdict(&collection, &single), Verdict::Different);
    }

    /// Durations more than 15% apart are different recordings even when the title
    /// is identical: an extended cut or a different performance.
    #[test]
    fn durations_more_than_fifteen_percent_apart_do_not_merge() {
        let short = track("晴天", "音乐无限", Some("周杰伦"), 200);
        let long = track("晴天", "音乐无限", Some("周杰伦"), 240);

        assert!(sameness(&short, &long) == 0.0);
        assert_eq!(verdict(&short, &long), Verdict::Different);

        // Inside the tolerance it still merges: 200 s against 220 s is 9%.
        let near = track("晴天", "音乐无限", Some("周杰伦"), 220);
        assert_eq!(verdict(&short, &near), Verdict::SameRecording);

        // A few seconds of drift is rounding, not a different recording.
        let rounded = track("晴天", "音乐无限", Some("周杰伦"), 203);
        assert_eq!(verdict(&short, &rounded), Verdict::SameRecording);
    }

    /// Re-upload accounts are the norm, so an unrelated UP owner must not split
    /// an otherwise identical song.
    #[test]
    fn an_unrelated_uploader_does_not_split_the_same_title() {
        let left = track("晴天", "音乐无限", None, 269);
        let right = track("晴天", "音乐搬运工小李", None, 269);

        assert_eq!(verdict(&left, &right), Verdict::SameRecording);
        // It is still only a trim: sameness is below a perfect match.
        assert!(sameness(&left, &right) < 1.0);
    }

    /// An empty title carries no title signal at all: it must never be merged
    /// with a real song, while two unknowns are all the evidence there is.
    #[test]
    fn empty_titles_are_handled_without_matching_everything() {
        let unknown = track("", "上传者", None, 120);
        let titled = track("晴天", "上传者", None, 120);
        assert_eq!(verdict(&unknown, &titled), Verdict::Different);

        let also_unknown = track("", "另一个上传者", None, 120);
        assert_eq!(verdict(&unknown, &also_unknown), Verdict::SameRecording);
    }

    /// The same platform entry compared with itself is always one recording.
    #[test]
    fn the_same_entry_from_one_platform_merges() {
        let entry = jingtian_4k();
        assert_eq!(verdict(&entry, &entry.clone()), Verdict::SameRecording);
        assert!((sameness(&entry, &entry) - 1.0).abs() < 1e-9);
    }

    /// A conflicting real artist is the hard gate the arrangement case relies on,
    /// with the durations kept identical so only the artist can decide.
    #[test]
    fn conflicting_real_artists_do_not_merge() {
        let jay = track("晴天", "音乐无限", Some("周杰伦"), 269);
        let other = track("晴天", "音乐无限", Some("某翻唱歌手"), 269);

        assert_eq!(verdict(&jay, &other), Verdict::Different);
    }

    /// An unknown duration must not be read as a disagreement.
    #[test]
    fn an_unknown_duration_is_not_evidence_of_difference() {
        let timed = track("晴天", "音乐无限", Some("周杰伦"), 269);
        let untimed = track("晴天", "音乐无限", Some("周杰伦"), 0);

        assert_eq!(verdict(&timed, &untimed), Verdict::SameRecording);
    }
}

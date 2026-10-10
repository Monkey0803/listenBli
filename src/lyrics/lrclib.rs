//! LRCLib: a community lyrics database, and the one provider that serves both
//! platforms.
//!
//! One GET, no key, and the document is already LRC — which this crate can parse —
//! so the work is not fetching but *choosing*: a search by track name alone returns
//! covers, live versions and the same song under three spellings. What picks the
//! right one is the length: a hit whose duration is a few seconds off the track's
//! is almost always the same recording, and one that is 40 seconds off is a remix
//! or a different edit of the same name.
//!
//! A Bilibili upload of a Western song has no Bilibili subtitle and may not be in
//! NetEase, but it is very likely to be here.

use serde::Deserialize;

use super::lrc::{self, Lyrics, LyricsSource};
use crate::api::client::{Api, ApiError};
use crate::api::models::Track;

/// How far a hit's duration may be from the track's, in seconds.
///
/// Generous on purpose: the length in a Bilibili title and the length LRCLib
/// reports are measured differently (intros, applause, an outro card), and being
/// strict here loses the right answer far more often than it saves a wrong one.
const DURATION_TOLERANCE_SECS: f64 = 8.0;

#[derive(Debug, Deserialize)]
struct Hit {
    #[serde(default, rename = "trackName")]
    track_name: String,
    #[serde(default, rename = "artistName")]
    artist_name: String,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default, rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

/// Look for synced lyrics for one track.
///
/// Only synced documents are accepted. LRCLib also serves plain text, but this app
/// highlights and scrolls by time, and inventing timestamps for an untimed document
/// would put the wrong line under the cursor — "no lyrics" is the honest answer.
pub fn fetch(api: &Api, track: &Track) -> Result<Option<Lyrics>, ApiError> {
    let title = lrclib_title(&track.title);
    if title.is_empty() {
        return Ok(None);
    }

    let mut url = url::Url::parse("https://lrclib.net/api/search").expect("a constant URL");
    url.query_pairs_mut()
        .append_pair("track_name", &title)
        .append_pair("artist_name", &lrclib_title(&track.author));
    let hits: Vec<Hit> = api.get_json_raw(url.as_str(), None)?;

    Ok(best(&hits, track).map(|hit| Lyrics {
        lines: lrc::parse_lrc(hit.synced_lyrics.as_deref().unwrap_or_default())
            .into_iter()
            .map(|(time, text)| lrc::LyricLine {
                time,
                text,
                translation: None,
            })
            .collect(),
        source: LyricsSource::Lrclib,
    }))
}

/// The cleanest query a noisy uploader title can give.
///
/// Bilibili titles carry decorations, and LRCLib matches on the literal string, so
/// the same cleaner the NetEase provider uses is applied here. Only the first
/// variant is used: the later ones drop words to widen the search, which is useful
/// when a search returns nothing, but LRCLib's own fuzzy matching already covers
/// that and a widened query is likelier to match a different song.
fn lrclib_title(title: &str) -> String {
    super::netease::search_queries(title)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// The hit whose length is closest to the track's.
fn best<'a>(hits: &'a [Hit], track: &Track) -> Option<&'a Hit> {
    let wanted = track.duration as f64;
    hits.iter()
        .filter(|hit| !hit.instrumental)
        .filter(|hit| {
            hit.synced_lyrics
                .as_deref()
                .is_some_and(|lyrics| !lyrics.trim().is_empty())
        })
        .filter(|hit| {
            // An unknown length on either side is not evidence against a hit.
            wanted == 0.0
                || hit
                    .duration
                    .is_none_or(|duration| (duration - wanted).abs() <= DURATION_TOLERANCE_SECS)
        })
        .min_by(|left, right| {
            // The artist decides before the length does. A cover band's recording
            // is the same length to within a second — measured in the fixture below
            // — but its timings are its own, so the official release has to win on
            // something other than duration.
            let rank = |hit: &Hit| (!artist_matches(hit, track), distance(hit, wanted));
            rank(left)
                .partial_cmp(&rank(right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Whether a hit's artist is named on the track.
///
/// The name is looked for in the title *and* the uploader, because a Bilibili
/// uploader is often not the artist but often does name them ("周杰伦 - 晴天" by
/// "音乐无限"). An unknown artist on either side is not held against a hit: the
/// length and LRCLib's own ordering are then what is left.
fn artist_matches(hit: &Hit, track: &Track) -> bool {
    let haystack = format!("{} {}", track.title, track.author).to_lowercase();
    let mut names = hit
        .artist_name
        .split(',')
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .peekable();
    if names.peek().is_none() {
        return true;
    }
    names.any(|name| haystack.contains(&name))
}

fn distance(hit: &Hit, wanted: f64) -> f64 {
    match hit.duration {
        Some(duration) if wanted > 0.0 => (duration - wanted).abs(),
        // No length on either side: fall back to the order LRCLib returned, which
        // puts its own best matches first.
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(duration: u64) -> Track {
        Track {
            bvid: "yt:test".into(),
            source: crate::api::models::Source::Youtube,
            aid: 0,
            cid: 0,
            title: "Never Gonna Give You Up".into(),
            author: "Rick Astley".into(),
            duration,
            cover: None,
        }
    }

    /// A trimmed search response: the official release, a cover, an instrumental,
    /// and a plain-text entry that must not be chosen.
    fn hits() -> Vec<Hit> {
        serde_json::from_str(
            r#"[
              { "trackName": "Never Gonna Give You Up", "artistName": "Rick Astley",
                "duration": 212.0, "syncedLyrics": "[00:19.32] We're no strangers to love",
                "instrumental": false },
              { "trackName": "Never Gonna Give You Up", "artistName": "Some Cover Band",
                "duration": 212.5, "syncedLyrics": "[00:20.00] a cover", "instrumental": false },
              { "trackName": "Never Gonna Give You Up", "artistName": "Karaoke",
                "duration": 211.0, "syncedLyrics": "[00:01.00] instrumental", "instrumental": true },
              { "trackName": "Never Gonna Give You Up", "artistName": "Rick Astley",
                "duration": 212.0, "plainLyrics": "untimed text", "instrumental": false },
              { "trackName": "Never Gonna Give You Up (Remix)", "artistName": "DJ",
                "duration": 380.0, "syncedLyrics": "[00:01.00] a remix", "instrumental": false }
            ]"#,
        )
        .expect("a valid fixture")
    }

    #[test]
    fn the_closest_synced_hit_wins() {
        let hits = hits();
        let chosen = best(&hits, &track(213)).expect("a hit");
        assert_eq!(chosen.artist_name, "Rick Astley");
        assert_eq!(chosen.duration, Some(212.0));
    }

    /// An instrumental has no lyrics to show, and a plain document has no times:
    /// neither may be chosen even when it is the closest match.
    #[test]
    fn instrumentals_and_untimed_documents_are_not_candidates() {
        let only_untimed: Vec<Hit> = serde_json::from_str(
            r#"[{ "trackName": "x", "artistName": "y", "duration": 213.0,
                  "plainLyrics": "text", "instrumental": false }]"#,
        )
        .unwrap();
        assert!(best(&only_untimed, &track(213)).is_none());

        let only_instrumental: Vec<Hit> = serde_json::from_str(
            r#"[{ "trackName": "x", "artistName": "y", "duration": 213.0,
                  "syncedLyrics": "[00:01.00] a", "instrumental": true }]"#,
        )
        .unwrap();
        assert!(best(&only_instrumental, &track(213)).is_none());

        // And the karaoke entry in the full fixture is skipped, not preferred for
        // being a second closer.
        let hits = hits();
        let chosen = best(&hits, &track(211)).expect("a hit");
        assert!(!chosen.instrumental);
    }

    /// Duration alone cannot separate the official release from a cover: this is
    /// the case that made the artist the first key (212.0 against a 213 s track
    /// loses to a cover's 212.5).
    #[test]
    fn the_artist_beats_a_closer_length() {
        let hits = hits();
        let chosen = best(&hits, &track(213)).expect("a hit");
        assert_eq!(
            chosen.artist_name, "Rick Astley",
            "the uploader names the artist, so the official release must win"
        );

        // With nothing naming the artist, the closest length takes over.
        let mut anonymous = track(213);
        anonymous.title = "女声翻唱合集".to_owned();
        anonymous.author = "音乐无限".to_owned();
        let chosen = best(&hits, &anonymous).expect("a hit");
        assert_eq!(chosen.artist_name, "Some Cover Band");
    }

    /// A hit whose length is nowhere near is a different recording of the same name.
    #[test]
    fn a_much_longer_recording_is_rejected() {
        let only_remix: Vec<Hit> = serde_json::from_str(
            r#"[{ "trackName": "Never Gonna Give You Up (Remix)", "artistName": "DJ",
                  "duration": 380.0, "syncedLyrics": "[00:01.00] a", "instrumental": false }]"#,
        )
        .unwrap();
        assert!(best(&only_remix, &track(213)).is_none());
        // With no length to compare against, it is all there is.
        assert!(best(&only_remix, &track(0)).is_some());
    }

    /// The query is the cleaned title, not the raw uploader string.
    #[test]
    fn the_query_is_cleaned() {
        assert_eq!(lrclib_title("晴天"), "晴天");
        // Decorations and the trailing artist are removed by the shared cleaner.
        let cleaned = lrclib_title("【4K修复】周杰伦 - 晴天");
        assert!(cleaned.contains("晴天"), "got {cleaned:?}");
        assert!(!cleaned.contains('【'), "got {cleaned:?}");
    }
}

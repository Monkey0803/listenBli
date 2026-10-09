//! Lyrics: Bilibili's own subtitle track first, NetEase as a fallback, with a
//! small on-disk cache so repeated plays do not re-query the third party.

pub mod bilibili;
pub mod lrc;
pub mod netease;

use std::path::PathBuf;

use crate::api::client::Api;
use crate::api::models::Track;
use crate::platform;

pub use lrc::{LyricLine, Lyrics, LyricsSource};

/// Resolve lyrics for `track`.
///
/// Order: Bilibili CC/AI subtitle -> NetEase -> empty. Never returns an error:
/// a missing lyric is a normal outcome, not a failure worth interrupting
/// playback for.
pub fn fetch_for(api: &Api, track: &Track) -> Lyrics {
    if let Some(cached) = load_cached(&track.bvid) {
        return cached;
    }

    if track.cid != 0 {
        match bilibili::fetch(api, &track.bvid, track.cid) {
            Ok(Some(lyrics)) => {
                store_cached(&track.bvid, &lyrics);
                return lyrics;
            }
            Ok(None) => {}
            Err(err) => eprintln!("bilibili subtitle for {} failed: {err}", track.bvid),
        }
    }

    match netease::fetch(api, track) {
        Ok(Some(lyrics)) => {
            store_cached(&track.bvid, &lyrics);
            lyrics
        }
        Ok(None) => Lyrics::empty(LyricsSource::None),
        Err(err) => {
            eprintln!("netease lyrics for {} failed: {err}", track.bvid);
            Lyrics::empty(LyricsSource::None)
        }
    }
}

fn cache_file(bvid: &str) -> Option<PathBuf> {
    let safe: String = bvid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        return None;
    }
    Some(
        platform::cache_dir()
            .join("lyrics")
            .join(format!("{safe}.json")),
    )
}

pub fn load_cached(bvid: &str) -> Option<Lyrics> {
    let path = cache_file(bvid)?;
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

pub fn store_cached(bvid: &str, lyrics: &Lyrics) {
    let Some(path) = cache_file(bvid) else { return };
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
            cache_file("../../etc/passwd").is_none()
                || !cache_file("../../etc/passwd")
                    .unwrap()
                    .to_string_lossy()
                    .contains("..")
        );
        assert!(cache_file("").is_none());
        assert!(cache_file("BV1xx411c7mD").is_some());
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

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
    serde_json::from_str::<Lyrics>(&text).ok()
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

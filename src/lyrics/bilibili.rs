//! Lyrics from the video's own CC/AI subtitle track.
//!
//! In practice most music uploads have no subtitle at all, which is why
//! `netease` exists as a fallback; this path is tried first because it is
//! Bilibili's own data and always matches the exact video timeline.

use super::lrc::{self, Lyrics, LyricsSource};
use crate::api::client::{Api, ApiError};
use crate::api::models::SubtitleBody;
use crate::api::video;

pub fn fetch(api: &Api, bvid: &str, cid: i64) -> Result<Option<Lyrics>, ApiError> {
    let items = video::subtitles(api, bvid, cid)?;
    let Some(item) = video::pick_subtitle(&items) else {
        return Ok(None);
    };

    let url = absolute_subtitle_url(&item.subtitle_url);
    let body: SubtitleBody = api.get_json_raw(&url, None)?;
    let lines = lrc::from_subtitle(&body.body);
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(Lyrics {
        lines,
        source: LyricsSource::BilibiliSubtitle,
    }))
}

/// Subtitle URLs come back protocol-relative (`//aisubtitle.hdslb.com/...`).
pub fn absolute_subtitle_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("//") {
        format!("https:{url}")
    } else if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_protocol_relative_urls_absolute() {
        assert_eq!(
            absolute_subtitle_url("//aisubtitle.hdslb.com/bfs/ai_subtitle/prod/x.json"),
            "https://aisubtitle.hdslb.com/bfs/ai_subtitle/prod/x.json"
        );
        assert_eq!(
            absolute_subtitle_url("https://a.example/x.json"),
            "https://a.example/x.json"
        );
        assert_eq!(
            absolute_subtitle_url("http://a.example/x.json"),
            "http://a.example/x.json"
        );
    }
}

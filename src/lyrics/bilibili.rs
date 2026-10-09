//! Lyrics from the video's own CC/AI subtitle track.
//!
//! In practice most music uploads have no subtitle at all, which is why
//! `netease` exists as a fallback. A *human-written* track is the best lyric
//! source there is — Bilibili's own data, written for this exact video, matching
//! its timeline exactly — so `mod::fetch_for` takes it unconditionally. A
//! *machine* track is a different animal entirely; see `machine_generated`.

use super::lrc::{self, Lyrics, LyricsSource};
use crate::api::client::{Api, ApiError};
use crate::api::models::{SubtitleBody, SubtitleItem};
use crate::api::video;

/// One fetched subtitle track, plus how much it can be trusted.
pub struct Subtitle {
    pub lyrics: Lyrics,
    /// Bilibili generated this with ASR rather than a human writing it.
    ///
    /// `mod::fetch_for` ranks this below NetEase, for two independent reasons:
    ///
    /// 1. ASR of *singing* is not a lyric. It is a phonetic guess at a melody,
    ///    which is why the output so often reads as fluent nonsense.
    /// 2. The track is not reliable evidence about *which* video it belongs to.
    ///    `/x/player/v2` has been observed handing back one video's AI subtitle
    ///    for another video's `cid` — a 4:15 esports documentary's transcript
    ///    served for an 8:14 concert upload, byte-identical to what a third,
    ///    unrelated `bvid` received.
    ///
    /// A human track carries neither problem, so it keeps the top slot.
    pub machine_generated: bool,
}

pub fn fetch(api: &Api, bvid: &str, cid: i64) -> Result<Option<Subtitle>, ApiError> {
    let items = video::subtitles(api, bvid, cid)?;
    let Some(item) = video::pick_subtitle(&items) else {
        return Ok(None);
    };
    let machine_generated = is_machine_generated(item);

    let url = absolute_subtitle_url(&item.subtitle_url);
    let body: SubtitleBody = api.get_json_raw(&url, None)?;
    let lines = lrc::from_subtitle(&body.body);
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(Subtitle {
        lyrics: Lyrics {
            lines,
            source: LyricsSource::BilibiliSubtitle,
        },
        machine_generated,
    }))
}

/// Did Bilibili's ASR write this track, or did a human?
///
/// Both signals are checked because they are not redundant: machine tracks are
/// normally named `ai-<lang>` *and* carry a non-zero `ai_type`, but responses
/// have been seen with only one of the two set, and either alone is enough to
/// disqualify a track from outranking a lyrics database.
pub fn is_machine_generated(item: &SubtitleItem) -> bool {
    item.ai_type != 0 || item.lan.to_ascii_lowercase().starts_with("ai-")
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

    fn item(lan: &str, ai_type: i64) -> SubtitleItem {
        SubtitleItem {
            lan: lan.to_string(),
            lan_doc: String::new(),
            subtitle_url: "//aisubtitle.hdslb.com/x.json".into(),
            ai_type,
        }
    }

    #[test]
    fn machine_tracks_are_recognised_by_name_or_flag() {
        for machine in [
            item("ai-zh", 1),
            item("ai-zh", 0),
            item("ai-en", 1),
            // The name is matched case-insensitively.
            item("AI-ZH", 0),
        ] {
            assert!(
                is_machine_generated(&machine),
                "{:?} should count as machine output",
                machine.lan
            );
        }
        for human in [item("zh-CN", 0), item("zh-Hans", 0), item("en-US", 0)] {
            assert!(
                !is_machine_generated(&human),
                "{:?} should count as human output",
                human.lan
            );
        }
        // A non-zero ai_type alone is enough, even under an odd name.
        assert!(is_machine_generated(&item("zh-CN", 1)));
    }

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

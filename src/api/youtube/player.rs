//! Resolving a video into a playable audio stream.
//!
//! ## What comes back
//!
//! `videoDetails` carries the metadata, `streamingData.adaptiveFormats` the
//! streams. Measured on 2026-10-10 with [`ClientKind::AndroidVr`], one video
//! offered:
//!
//! | itag | mime | bitrate | decodable here |
//! |---|---|---|---|
//! | 139 | `audio/mp4; codecs="mp4a.40.5"` (HE-AAC) | ~50k | yes |
//! | 140 | `audio/mp4; codecs="mp4a.40.2"` (AAC-LC) | ~130k | yes |
//! | 249 | `audio/webm; codecs="opus"` | ~50k | **no** |
//! | 251 | `audio/webm; codecs="opus"` | ~137k | **no** |
//!
//! Symphonia 0.5 — this app's decoder — has no Opus codec, so WebM/Opus entries are
//! skipped rather than handed to the engine to fail on. That makes the picker a
//! one-line rule: the best `audio/mp4` entry.
//!
//! ## The container
//!
//! The picked stream is `ftyp` + `moov` + `sidx` + `moof/mdat` — a fragmented MP4,
//! byte-for-byte the same *shape* as Bilibili's DASH audio. The streaming reader,
//! the cache and the `.m4a` export therefore apply unchanged; the only difference is
//! that its bytes must be requested in ranges (see the download plan in `net`).

use serde_json::Value;

use super::innertube::{ClientKind, InnerTube, YoutubeError};
use crate::api::models::AudioQuality;

/// One audio stream YouTube offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPick {
    /// The DASH `itag`, which is what names the cache file.
    pub tag: u32,
    pub quality: AudioQuality,
    /// The direct `googlevideo.com` URL. Signed and short-lived: it is tied to the
    /// requesting IP and expires, so it is never cached.
    pub url: String,
    /// Advertised total size, used as the cache's expected length.
    pub content_length: Option<u64>,
    pub bitrate: u64,
    /// End of the init segment (`ftyp` + `moov`) and of the `sidx`.
    ///
    /// The decoder can be handed nothing until it has both, so the first ranged
    /// request covers them and the opening of the media data. `None` when the
    /// response did not say, in which case the downloader falls back to a plain
    /// opening chunk.
    pub init_end: Option<u64>,
    pub index_end: Option<u64>,
}

/// Everything needed to play one video.
#[derive(Debug, Clone)]
pub struct Playable {
    pub title: String,
    pub author: String,
    /// Seconds, from `videoDetails.lengthSeconds`.
    pub duration: u64,
    /// A stream that is still running has no length and cannot be cached or
    /// seeked, so it is refused before anything is downloaded.
    pub is_live: bool,
    pub picks: Vec<AudioPick>,
}

impl Playable {
    /// The stream to play: the best one this app can actually decode.
    pub fn best_audio(&self) -> Option<&AudioPick> {
        pick_audio(&self.picks)
    }
}

/// Resolve one video.
///
/// `client` is explicit because which identity to ask is a policy decision (the
/// web client is refused playback entirely), and the caller is the one that knows
/// which fallbacks have already been tried.
pub fn resolve(
    http: &InnerTube,
    client: ClientKind,
    video_id: &str,
) -> Result<Playable, YoutubeError> {
    let response = http.post(
        client,
        "player",
        serde_json::json!({
            "videoId": video_id,
            // Both are required by some clients; sending them costs nothing.
            "contentCheckOk": true,
            "racyCheckOk": true,
        }),
    )?;
    parse(&response)
}

/// Read a `player` response.
///
/// A refusal is reported with YouTube's own words: "Video unavailable",
/// "This video is not available in your country", "Sign in to confirm you're not a
/// bot" are all actionable to a person and none of them are guessable, so they are
/// passed through rather than mapped onto a generic message.
pub fn parse(response: &Value) -> Result<Playable, YoutubeError> {
    let status = response
        .get("playabilityStatus")
        .and_then(|status| status.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    if status != "OK" {
        let reason = response
            .get("playabilityStatus")
            .and_then(|status| status.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("YouTube 未给出原因");
        return Err(YoutubeError::Api(format!(
            "无法播放该视频：{reason}（{status}）"
        )));
    }

    let details = response
        .get("videoDetails")
        .ok_or_else(|| YoutubeError::Decode("缺少 videoDetails".to_owned()))?;
    let title = details
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let author = details
        .get("author")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let duration = details
        .get("lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(0);
    let is_live = details
        .get("isLive")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || details
            .get("isLiveContent")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    let formats = response
        .get("streamingData")
        .and_then(|data| data.get("adaptiveFormats"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(Playable {
        title,
        author,
        duration,
        is_live,
        picks: formats.iter().filter_map(audio_pick).collect(),
    })
}

/// One `adaptiveFormats` entry, when it is audio this app can decode.
fn audio_pick(format: &Value) -> Option<AudioPick> {
    let mime = format.get("mimeType").and_then(Value::as_str)?;
    // The whole decoder decision, in one line: AAC in MP4 only.
    if !mime.starts_with("audio/mp4") {
        return None;
    }
    let url = format.get("url").and_then(Value::as_str)?;
    let tag = format.get("itag").and_then(Value::as_u64)? as u32;
    let quality = match tag {
        140 => AudioQuality::YtAac128,
        139 => AudioQuality::YtAac48,
        // A future AAC stream: still decodable, so take it and label it by codec.
        _ => AudioQuality::YtAac128,
    };
    Some(AudioPick {
        tag,
        quality,
        url: url.to_owned(),
        content_length: format
            .get("contentLength")
            .and_then(Value::as_str)
            .and_then(|len| len.parse().ok()),
        bitrate: format.get("bitrate").and_then(Value::as_u64).unwrap_or(0),
        init_end: range_end(format, "initRange"),
        index_end: range_end(format, "indexRange"),
    })
}

/// The inclusive end offset of one of the response's byte ranges.
fn range_end(format: &Value, field: &str) -> Option<u64> {
    format
        .get(field)
        .and_then(|range| range.get("end"))
        .and_then(Value::as_str)
        .and_then(|end| end.parse().ok())
}

/// The best pick: highest bitrate among the decodable ones.
pub fn pick_audio(picks: &[AudioPick]) -> Option<&AudioPick> {
    picks.iter().max_by_key(|pick| pick.bitrate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A trimmed `player` response with the four streams that were really offered.
    fn playable() -> Value {
        json!({
            "playabilityStatus": { "status": "OK" },
            "videoDetails": {
                "title": "Rick Astley - Never Gonna Give You Up",
                "author": "Rick Astley",
                "lengthSeconds": "213",
                "isLive": false,
                "thumbnail": { "thumbnails": [{ "url": "https://i.ytimg.com/vi/x/hqdefault.jpg" }] }
            },
            "streamingData": { "adaptiveFormats": [
                { "itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 136544,
                  "url": "https://example.invalid/opus-hi" },
                { "itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 130677,
                  "url": "https://example.invalid/aac-hi", "contentLength": "3449447",
                  "initRange": { "start": "0", "end": "722" },
                  "indexRange": { "start": "723", "end": "1018" } },
                { "itag": 249, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 49496,
                  "url": "https://example.invalid/opus-lo" },
                { "itag": 139, "mimeType": "audio/mp4; codecs=\"mp4a.40.5\"", "bitrate": 50152,
                  "url": "https://example.invalid/aac-lo", "contentLength": "1300631" }
            ]}
        })
    }

    #[test]
    fn opus_is_skipped_because_the_decoder_has_no_codec_for_it() {
        let playable = parse(&playable()).expect("a playable response");
        assert_eq!(playable.title, "Rick Astley - Never Gonna Give You Up");
        assert_eq!(playable.author, "Rick Astley");
        assert_eq!(playable.duration, 213);
        assert!(!playable.is_live);

        // The two Opus entries are gone; the two AAC ones remain.
        assert_eq!(playable.picks.len(), 2);
        assert!(playable.picks.iter().all(|pick| pick.url.contains("aac")));

        let best = playable.best_audio().expect("something to play");
        assert_eq!(best.tag, 140);
        assert_eq!(best.quality, AudioQuality::YtAac128);
        assert_eq!(best.content_length, Some(3_449_447));
        // The first ranged request needs these to cover the container header.
        assert_eq!(best.init_end, Some(722));
        assert_eq!(best.index_end, Some(1018));
    }

    /// Nothing decodable must be a clear refusal, not a silent empty playback.
    #[test]
    fn a_webm_only_video_has_nothing_to_play() {
        let response = json!({
            "playabilityStatus": { "status": "OK" },
            "videoDetails": { "title": "t", "author": "a", "lengthSeconds": "10" },
            "streamingData": { "adaptiveFormats": [
                { "itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 1,
                  "url": "https://example.invalid/opus" }
            ]}
        });
        let playable = parse(&response).expect("the response itself is fine");
        assert!(playable.best_audio().is_none());
    }

    /// YouTube's own reason is what makes a refusal actionable, so it must survive.
    #[test]
    fn a_refusal_is_reported_in_youtubes_words() {
        let response = json!({
            "playabilityStatus": {
                "status": "UNPLAYABLE",
                "reason": "This video is not available in your country"
            }
        });
        let err = parse(&response).unwrap_err().to_string();
        assert!(err.contains("This video is not available"), "got {err}");
        assert!(err.contains("UNPLAYABLE"), "got {err}");
    }

    #[test]
    fn a_live_stream_is_flagged_so_it_can_be_refused() {
        let response = json!({
            "playabilityStatus": { "status": "OK" },
            "videoDetails": { "title": "live", "author": "a", "isLive": true },
            "streamingData": { "adaptiveFormats": [] }
        });
        assert!(parse(&response).unwrap().is_live);
    }

    #[test]
    fn a_missing_video_details_is_a_decode_error() {
        let response = json!({ "playabilityStatus": { "status": "OK" } });
        assert!(matches!(parse(&response), Err(YoutubeError::Decode(_))));
    }
}

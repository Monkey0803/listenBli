//! Video metadata, audio stream selection and CC subtitles.

use super::client::{Api, ApiError};
use super::models::{AudioQuality, PlayUrlData, PlayerV2Data, SubtitleItem, Track, ViewData};
use crate::util;

/// A decodable audio stream plus its fallback URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSource {
    pub quality: AudioQuality,
    /// Primary URL first, then `backupUrl` entries.
    pub urls: Vec<String>,
}

impl AudioSource {
    pub fn primary(&self) -> &str {
        self.urls.first().map(String::as_str).unwrap_or("")
    }
}

/// Preference order for the DASH audio array.
///
/// `30280` and `30232` are AAC-LC (`mp4a.40.2`), which Symphonia decodes
/// properly. `30216` is HE-AAC (`mp4a.40.5`); Symphonia parses its SBR metadata
/// but does not apply SBR, so it plays back at the 24 kHz core rate. It is
/// therefore only used when nothing else is offered.
const AUDIO_PREFERENCE: [(u32, AudioQuality); 3] = [
    (30280, AudioQuality::K192),
    (30232, AudioQuality::K132),
    (30216, AudioQuality::K64),
];

/// Dolby (EC-3) is deliberately never selected: no Rust decoder here handles it.
pub fn pick_audio(data: &PlayUrlData, prefer_flac: bool) -> Option<AudioSource> {
    let dash = data.dash.as_ref()?;

    if prefer_flac {
        if let Some(flac) = dash
            .flac
            .as_ref()
            .and_then(|f| f.audio.as_ref())
            .filter(|f| !f.url().is_empty())
        {
            let mut urls = vec![flac.url().to_string()];
            urls.extend(flac.backups().iter().cloned());
            return Some(AudioSource {
                quality: AudioQuality::Flac,
                urls,
            });
        }
    }

    let audios = dash.audio.as_ref()?;

    for (id, quality) in AUDIO_PREFERENCE {
        if let Some(media) = audios.iter().find(|m| m.id == id && !m.url().is_empty()) {
            let mut urls = vec![media.url().to_string()];
            urls.extend(media.backups().iter().cloned());
            return Some(AudioSource { quality, urls });
        }
    }

    // Unknown ids: take the highest bandwidth that is not HE-AAC.
    let mut candidates: Vec<_> = audios
        .iter()
        .filter(|m| !m.url().is_empty() && !is_he_aac(m))
        .collect();
    candidates.sort_by_key(|m| std::cmp::Reverse(m.bandwidth));
    candidates.first().map(|media| {
        let mut urls = vec![media.url().to_string()];
        urls.extend(media.backups().iter().cloned());
        AudioSource {
            quality: AudioQuality::K132,
            urls,
        }
    })
}

fn is_he_aac(media: &super::models::DashMedia) -> bool {
    media.codecs.contains("mp4a.40.5") || media.codecs.contains("mp4a.40.29")
}

pub fn describe_streams(data: &PlayUrlData) -> String {
    let Some(dash) = data.dash.as_ref() else {
        return "无音频流".to_string();
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(flac) = &dash.flac {
        if flac.audio.is_some() {
            parts.push("无损".to_string());
        }
    }
    if let Some(audios) = &dash.audio {
        for a in audios {
            parts.push(format!("{} ({})", a.id, a.codecs));
        }
    }
    parts.join(", ")
}

pub fn view(api: &Api, bvid: &str) -> Result<ViewData, ApiError> {
    api.get_bili(&format!("/x/web-interface/view?bvid={bvid}"))
}

/// Fill in `cid`, `author`, `duration` and `cover` when a `Track` came from a
/// source that does not carry them (history/favourites entries, pasted URLs).
pub fn resolve_track(api: &Api, track: &mut Track) -> Result<(), ApiError> {
    if track.cid != 0 && !track.title.is_empty() {
        return Ok(());
    }
    let data = view(api, &track.bvid)?;
    track.cid = data.cid;
    track.aid = data.aid;
    if track.title.is_empty() {
        track.title = data.title;
    }
    if track.author.is_empty() {
        if let Some(owner) = &data.owner {
            track.author = owner.name.clone();
        }
    }
    if track.duration == 0 {
        track.duration = data.duration;
    }
    if track.cover.is_none() {
        track.cover = util::normalize_url(&data.pic);
    }
    Ok(())
}

pub fn playurl(api: &Api, bvid: &str, cid: i64) -> Result<PlayUrlData, ApiError> {
    let params = vec![
        ("bvid".to_string(), bvid.to_string()),
        ("cid".to_string(), cid.to_string()),
        // 4048 requests DASH plus every premium stream; unsupported ones are
        // simply absent from the response.
        ("fnval".to_string(), "4048".to_string()),
        ("fnver".to_string(), "0".to_string()),
        ("fourk".to_string(), "1".to_string()),
    ];
    api.get_bili_signed("/x/player/playurl", &params)
}

pub fn subtitles(api: &Api, bvid: &str, cid: i64) -> Result<Vec<SubtitleItem>, ApiError> {
    let data: PlayerV2Data = api.get_bili(&format!("/x/player/v2?bvid={bvid}&cid={cid}"))?;
    Ok(data.subtitle.and_then(|s| s.subtitles).unwrap_or_default())
}

/// Rank subtitles so a human-readable Chinese track wins over machine output.
pub fn pick_subtitle(items: &[SubtitleItem]) -> Option<&SubtitleItem> {
    let score = |item: &SubtitleItem| -> i32 {
        let lan = item.lan.to_ascii_lowercase();
        match lan.as_str() {
            "zh-cn" | "zh-hans" => 100,
            "ai-zh" => 90,
            "zh-tw" | "zh-hant" => 70,
            other if other.starts_with("zh") => 60,
            other if other.starts_with("ai-") => 40,
            _ => 10,
        }
    };
    items
        .iter()
        .filter(|item| !item.subtitle_url.is_empty())
        .max_by_key(|item| score(item))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{Dash, DashMedia, FlacAudio, FlacMedia};

    fn media(id: u32, codecs: &str, bandwidth: u64) -> DashMedia {
        DashMedia {
            id,
            base_url: format!("https://cdn/{id}.m4s"),
            base_url_snake: String::new(),
            backup_url: vec![format!("https://backup/{id}.m4s")],
            backup_url_snake: Vec::new(),
            bandwidth,
            codecs: codecs.to_string(),
        }
    }

    /// Regression test: Bilibili sends `baseUrl` *and* `base_url` in the same
    /// object. An earlier `#[serde(alias)]` made this fail with "duplicate field
    /// `baseUrl`", which broke playback completely.
    #[test]
    fn dash_media_tolerates_both_url_spellings() {
        let json = r#"{
            "id": 30280,
            "baseUrl": "https://cdn/primary.m4s",
            "base_url": "https://cdn/primary.m4s",
            "backupUrl": ["https://backup/a.m4s"],
            "backup_url": ["https://backup/a.m4s"],
            "bandwidth": 203786,
            "codecs": "mp4a.40.2"
        }"#;
        let media: DashMedia = serde_json::from_str(json).expect("both spellings must deserialize");
        assert_eq!(media.url(), "https://cdn/primary.m4s");
        assert_eq!(media.backups(), ["https://backup/a.m4s"]);
    }

    #[test]
    fn dash_media_falls_back_to_snake_case() {
        let json = r#"{"id": 30232, "base_url": "https://cdn/snake.m4s", "backup_url": ["https://backup/b.m4s"]}"#;
        let media: DashMedia = serde_json::from_str(json).unwrap();
        assert_eq!(media.url(), "https://cdn/snake.m4s");
        assert_eq!(media.backups(), ["https://backup/b.m4s"]);
    }

    fn data_with(audios: Vec<DashMedia>, flac: Option<FlacMedia>) -> PlayUrlData {
        PlayUrlData {
            dash: Some(Dash {
                audio: Some(audios),
                flac: flac.map(|audio| FlacAudio { audio: Some(audio) }),
                dolby: None,
            }),
            timelength: Some(213_000),
        }
    }

    #[test]
    fn prefers_192k_aac_lc() {
        let data = data_with(
            vec![
                media(30216, "mp4a.40.5", 43_962),
                media(30232, "mp4a.40.2", 102_931),
                media(30280, "mp4a.40.2", 203_786),
            ],
            None,
        );
        let picked = pick_audio(&data, false).unwrap();
        assert_eq!(picked.quality, AudioQuality::K192);
        assert_eq!(picked.primary(), "https://cdn/30280.m4s");
        assert_eq!(picked.urls.len(), 2, "backup url should be kept");
    }

    #[test]
    fn he_aac_is_only_used_as_a_last_resort() {
        let data = data_with(vec![media(30216, "mp4a.40.5", 43_962)], None);
        let picked = pick_audio(&data, false).unwrap();
        assert_eq!(picked.quality, AudioQuality::K64);
    }

    #[test]
    fn flac_is_used_only_when_opted_in() {
        let flac = FlacMedia {
            base_url: "https://cdn/flac.m4s".into(),
            base_url_snake: String::new(),
            backup_url: vec![],
            backup_url_snake: vec![],
        };
        let data = data_with(vec![media(30280, "mp4a.40.2", 203_786)], Some(flac));
        assert_eq!(
            pick_audio(&data, false).unwrap().quality,
            AudioQuality::K192
        );
        assert_eq!(pick_audio(&data, true).unwrap().quality, AudioQuality::Flac);
    }

    #[test]
    fn dolby_is_never_selected() {
        let mut data = data_with(vec![media(30232, "mp4a.40.2", 102_931)], None);
        if let Some(dash) = data.dash.as_mut() {
            dash.dolby = Some(crate::api::models::DolbyAudio {
                audio: Some(media(30250, "ec-3", 448_000)),
            });
        }
        assert_eq!(
            pick_audio(&data, false).unwrap().quality,
            AudioQuality::K132
        );
    }

    #[test]
    fn no_dash_yields_nothing() {
        let data = PlayUrlData {
            dash: None,
            timelength: None,
        };
        assert!(pick_audio(&data, false).is_none());
    }

    #[test]
    fn unknown_ids_fall_back_to_highest_non_he_aac_bandwidth() {
        let data = data_with(
            vec![
                media(99999, "mp4a.40.5", 500_000),
                media(88888, "mp4a.40.2", 300_000),
                media(77777, "mp4a.40.2", 100_000),
            ],
            None,
        );
        assert_eq!(
            pick_audio(&data, false).unwrap().primary(),
            "https://cdn/88888.m4s"
        );
    }

    #[test]
    fn subtitle_preference_order() {
        let mk = |lan: &str, ai: i64| SubtitleItem {
            lan: lan.to_string(),
            lan_doc: String::new(),
            subtitle_url: "//aisubtitle.hdslb.com/x.json".into(),
            ai_type: ai,
        };
        let items = vec![mk("ai-zh", 1), mk("zh-CN", 0), mk("en-US", 0)];
        assert_eq!(pick_subtitle(&items).unwrap().lan, "zh-CN");

        let only_ai = vec![mk("ai-zh", 1), mk("en-US", 0)];
        assert_eq!(pick_subtitle(&only_ai).unwrap().lan, "ai-zh");
    }

    #[test]
    fn subtitles_without_url_are_ignored() {
        let empty = SubtitleItem {
            lan: "zh-CN".into(),
            lan_doc: String::new(),
            subtitle_url: String::new(),
            ai_type: 0,
        };
        assert!(pick_subtitle(&[empty]).is_none());
    }
}

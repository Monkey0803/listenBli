//! Serde models for the Bilibili and NetEase responses we consume.
//!
//! Field names mirror the wire format (camelCase in several places), and every
//! field that has been observed to be absent on some responses is `Option` or
//! `#[serde(default)]` so a schema drift degrades instead of panicking.

use serde::{Deserialize, Serialize};

/// A playable item, normalized from search results, favourites, history or a URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub bvid: String,
    #[serde(default)]
    pub aid: i64,
    #[serde(default)]
    pub cid: i64,
    pub title: String,
    #[serde(default)]
    pub author: String,
    /// Seconds.
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub cover: Option<String>,
}

impl Track {
    pub fn key(&self) -> String {
        if self.bvid.is_empty() {
            format!("aid{}", self.aid)
        } else {
            self.bvid.clone()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserInfo {
    pub mid: i64,
    pub uname: String,
    pub face: String,
    pub vip_status: i64,
}

impl UserInfo {
    pub fn is_vip(&self) -> bool {
        self.vip_status > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioQuality {
    Flac,
    K192,
    K132,
    K64,
}

impl AudioQuality {
    pub fn label(self) -> &'static str {
        match self {
            AudioQuality::Flac => "无损 FLAC",
            AudioQuality::K192 => "192K",
            AudioQuality::K132 => "132K",
            AudioQuality::K64 => "64K (低清)",
        }
    }

    /// The `dash.audio[].id` this quality corresponds to; 0 for FLAC because it
    /// is reported under `dash.flac` rather than in the `audio` array.
    pub fn stream_id(self) -> u32 {
        match self {
            AudioQuality::Flac => 0,
            AudioQuality::K192 => 30280,
            AudioQuality::K132 => 30232,
            AudioQuality::K64 => 30216,
        }
    }
}

// ---------------------------------------------------------------------------
// nav
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct NavData {
    #[serde(rename = "isLogin", default)]
    pub is_login: bool,
    #[serde(default)]
    pub mid: i64,
    #[serde(default)]
    pub uname: String,
    #[serde(default)]
    pub face: String,
    #[serde(rename = "vipStatus", default)]
    pub vip_status: i64,
    #[serde(rename = "wbi_img", default)]
    pub wbi_img: Option<WbiImg>,
}

#[derive(Debug, Deserialize)]
pub struct WbiImg {
    #[serde(rename = "img_url", default)]
    pub img_url: String,
    #[serde(rename = "sub_url", default)]
    pub sub_url: String,
}

// ---------------------------------------------------------------------------
// finger/spi
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SpiData {
    #[serde(default)]
    pub b_3: String,
    #[serde(default)]
    pub b_4: String,
}

// ---------------------------------------------------------------------------
// video view
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ViewData {
    #[serde(default)]
    pub bvid: String,
    #[serde(default)]
    pub aid: i64,
    #[serde(default)]
    pub cid: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub pic: String,
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub owner: Option<ViewOwner>,
}

#[derive(Debug, Deserialize)]
pub struct ViewOwner {
    #[serde(default)]
    pub name: String,
}

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchTypeData {
    #[serde(default)]
    pub result: Vec<SearchItem>,
    /// Total pages the query has; 0 when the server does not say.
    #[serde(rename = "numPages", default)]
    pub num_pages: u32,
}

#[derive(Debug, Deserialize)]
pub struct SearchItem {
    #[serde(default)]
    pub bvid: String,
    #[serde(default)]
    pub aid: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    /// `"5:17"` or `"1:02:03"`.
    #[serde(default)]
    pub duration: String,
    #[serde(default)]
    pub pic: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchAllData {
    #[serde(default)]
    pub result: Vec<SearchAllGroup>,
    /// Total pages the query has; 0 when the server does not say.
    #[serde(rename = "numPages", default)]
    pub num_pages: u32,
}

#[derive(Debug, Deserialize)]
pub struct SearchAllGroup {
    #[serde(rename = "result_type", default)]
    pub result_type: String,
    #[serde(default)]
    pub data: Vec<SearchItem>,
}

// ---------------------------------------------------------------------------
// playurl
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PlayUrlData {
    #[serde(default)]
    pub dash: Option<Dash>,
    #[serde(default)]
    pub timelength: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Dash {
    #[serde(default)]
    pub audio: Option<Vec<DashMedia>>,
    #[serde(default)]
    pub flac: Option<FlacAudio>,
    #[serde(default)]
    pub dolby: Option<DolbyAudio>,
}

/// One DASH media entry.
///
/// Bilibili sends *both* `baseUrl` and `base_url` in the same object. A serde
/// `alias` cannot be used to accept either spelling, because that maps two JSON
/// keys onto one field and fails with "duplicate field". Distinct fields plus an
/// accessor is the safe way to tolerate both shapes.
#[derive(Debug, Deserialize)]
pub struct DashMedia {
    #[serde(default)]
    pub id: u32,
    #[serde(rename = "baseUrl", default)]
    pub base_url: String,
    #[serde(rename = "base_url", default)]
    pub base_url_snake: String,
    #[serde(rename = "backupUrl", default)]
    pub backup_url: Vec<String>,
    #[serde(rename = "backup_url", default)]
    pub backup_url_snake: Vec<String>,
    #[serde(default)]
    pub bandwidth: u64,
    #[serde(default)]
    pub codecs: String,
}

impl DashMedia {
    pub fn url(&self) -> &str {
        if self.base_url.is_empty() {
            &self.base_url_snake
        } else {
            &self.base_url
        }
    }

    pub fn backups(&self) -> &[String] {
        if self.backup_url.is_empty() {
            &self.backup_url_snake
        } else {
            &self.backup_url
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FlacAudio {
    #[serde(default)]
    pub audio: Option<FlacMedia>,
}

#[derive(Debug, Deserialize)]
pub struct FlacMedia {
    #[serde(rename = "baseUrl", default)]
    pub base_url: String,
    #[serde(rename = "base_url", default)]
    pub base_url_snake: String,
    #[serde(rename = "backupUrl", default)]
    pub backup_url: Vec<String>,
    #[serde(rename = "backup_url", default)]
    pub backup_url_snake: Vec<String>,
}

impl FlacMedia {
    pub fn url(&self) -> &str {
        if self.base_url.is_empty() {
            &self.base_url_snake
        } else {
            &self.base_url
        }
    }

    pub fn backups(&self) -> &[String] {
        if self.backup_url.is_empty() {
            &self.backup_url_snake
        } else {
            &self.backup_url
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DolbyAudio {
    #[serde(default)]
    pub audio: Option<DashMedia>,
}

// ---------------------------------------------------------------------------
// player/v2  (subtitles == lyrics when the uploader supplied them)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PlayerV2Data {
    #[serde(default)]
    pub subtitle: Option<SubtitleInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SubtitleInfo {
    #[serde(default)]
    pub subtitles: Option<Vec<SubtitleItem>>,
}

#[derive(Debug, Deserialize)]
pub struct SubtitleItem {
    #[serde(default)]
    pub lan: String,
    #[serde(rename = "lan_doc", default)]
    pub lan_doc: String,
    #[serde(rename = "subtitle_url", default)]
    pub subtitle_url: String,
    #[serde(rename = "ai_type", default)]
    pub ai_type: i64,
}

#[derive(Debug, Deserialize)]
pub struct SubtitleBody {
    #[serde(default)]
    pub body: Vec<SubtitleLine>,
}

#[derive(Debug, Deserialize)]
pub struct SubtitleLine {
    #[serde(default)]
    pub from: f64,
    #[serde(default)]
    pub to: f64,
    #[serde(default)]
    pub content: String,
}

// ---------------------------------------------------------------------------
// login
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct QrGenerateData {
    #[serde(default)]
    pub url: String,
    #[serde(rename = "qrcode_key", default)]
    pub qrcode_key: String,
}

#[derive(Debug, Deserialize)]
pub struct QrPollData {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub message: String,
}

// ---------------------------------------------------------------------------
// favourites & history
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct FavFolder {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "media_count", default)]
    pub media_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct FavFolderListData {
    #[serde(default)]
    pub list: Vec<FavFolder>,
}

#[derive(Debug, Deserialize)]
pub struct FavResourceListData {
    #[serde(default)]
    pub medias: Option<Vec<FavMedia>>,
    #[serde(rename = "has_more", default)]
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct FavMedia {
    #[serde(default)]
    pub bvid: String,
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub upper: Option<FavUpper>,
    /// `1` = video, `2` = audio, `12` = audio collection. Only videos are played.
    #[serde(default)]
    pub attr: i64,
}

#[derive(Debug, Deserialize)]
pub struct FavUpper {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryData {
    #[serde(default)]
    pub list: Vec<HistoryItem>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryItem {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub duration: u64,
    #[serde(rename = "author_name", default)]
    pub author_name: String,
    #[serde(default)]
    pub history: Option<HistoryRef>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryRef {
    #[serde(default)]
    pub bvid: String,
    #[serde(default)]
    pub cid: i64,
    #[serde(default)]
    pub oid: i64,
}

// ---------------------------------------------------------------------------
// NetEase lyrics
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct NeSearchResult {
    #[serde(default)]
    pub result: Option<NeSearchInner>,
}

#[derive(Debug, Deserialize)]
pub struct NeSearchInner {
    #[serde(default)]
    pub songs: Vec<NeSong>,
}

#[derive(Debug, Deserialize)]
pub struct NeSong {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub artists: Vec<NeArtist>,
    #[serde(default)]
    pub album: Option<NeAlbum>,
    /// Milliseconds.
    #[serde(default)]
    pub duration: u64,
}

#[derive(Debug, Deserialize)]
pub struct NeArtist {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct NeAlbum {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct NeLyric {
    #[serde(default)]
    pub lrc: Option<NeLyricText>,
    #[serde(default)]
    pub tlyric: Option<NeLyricText>,
}

#[derive(Debug, Deserialize)]
pub struct NeLyricText {
    #[serde(default)]
    pub lyric: String,
}

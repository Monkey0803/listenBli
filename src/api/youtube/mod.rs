//! The YouTube source: search, pagination and stream resolution.
//!
//! One instance is shared by the API worker. It owns the connection pool and the
//! page cursors, so the worker's page-based search loop stays page-based while
//! YouTube's opaque tokens stay in here.

pub mod innertube;
pub mod player;
pub mod search;

use std::collections::HashMap;
use std::sync::Mutex;

pub use innertube::{ClientKind, InnerTube, YoutubeError};
pub use player::{AudioPick, Playable};
pub use search::SearchPage;

use crate::api::models::Track;

/// YouTube, as the rest of the app sees it.
pub struct Youtube {
    http: InnerTube,
    /// `keyword` → the token that fetches its next page.
    ///
    /// A continuation token is only valid for the query it came from, and the app
    /// asks for pages by number, so the token is remembered here and consumed in
    /// order.
    cursors: Mutex<HashMap<String, String>>,
}

impl Youtube {
    pub fn new() -> Self {
        Self::build(InnerTube::new())
    }

    #[cfg(test)]
    fn with_api_key(api_key: impl Into<String>) -> Self {
        Self::build(InnerTube::with_api_key(api_key))
    }

    fn build(http: InnerTube) -> Self {
        Self {
            http,
            cursors: Mutex::new(HashMap::new()),
        }
    }

    /// One page of results, and whether another page exists.
    pub fn search(&self, keyword: &str, page: u32) -> Result<(Vec<Track>, bool), YoutubeError> {
        let continuation = self.take_cursor(keyword, page)?;
        let page_result = search::search(&self.http, keyword, continuation.as_deref())?;
        let has_more = page_result.continuation.is_some();
        self.store_cursor(keyword, page_result.continuation);
        Ok((page_result.tracks, has_more))
    }

    /// Resolve one video into a playable stream.
    ///
    /// Tries each playback client in turn: the list is documented in
    /// [`ClientKind::PLAYBACK_FALLBACKS`], and the *last* error is what the caller
    /// sees, because a refusal from the client that got furthest is the informative
    /// one.
    pub fn resolve(&self, video_id: &str) -> Result<Playable, YoutubeError> {
        let mut last_error = None;
        for client in ClientKind::PLAYBACK_FALLBACKS {
            match player::resolve(&self.http, *client, video_id) {
                Ok(playable) => return Ok(playable),
                Err(err) => {
                    eprintln!("youtube player via {client:?} failed for {video_id}: {err}");
                    last_error = Some(err);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| YoutubeError::Api("没有可用的播放客户端".to_owned())))
    }

    /// The caption tracks the player offers for a video.
    ///
    /// A second `player` call rather than a shared cache: the streaming URLs it also
    /// returns expire in minutes, so caching the response would only serve stale
    /// ones. The caption URLs are signed the same way and are used immediately.
    pub fn captions(
        &self,
        video_id: &str,
    ) -> Result<Vec<crate::lyrics::captions::CaptionTrack>, YoutubeError> {
        // The playback clients, not `WEB`: a `player` response that is UNPLAYABLE
        // carries no `captions` object at all, and `WEB` is refused playback — so
        // asking it is a guaranteed empty answer. Measured: `ANDROID_VR` lists six
        // tracks for the same video where `WEB` lists none.
        let mut last_error = None;
        for client in ClientKind::PLAYBACK_FALLBACKS {
            match self.http.post(
                *client,
                "player",
                serde_json::json!({ "videoId": video_id }),
            ) {
                Ok(response) => {
                    let tracks = parse_captions(&response);
                    if !tracks.is_empty() {
                        return Ok(tracks);
                    }
                }
                Err(err) => {
                    eprintln!("youtube captions via {client:?} failed for {video_id}: {err}");
                    last_error = Some(err);
                }
            }
        }
        // No captions is the normal case for music, not a failure.
        if let Some(err) = last_error {
            eprintln!("youtube had no caption list for {video_id} ({err})");
        }
        Ok(Vec::new())
    }

    /// The subtitle document for a video, and whether it is auto-generated.
    ///
    /// Both halves have to use the *same* client: the track list and the URL inside
    /// it are minted for one identity, and fetching with another returns an empty
    /// document.
    pub fn caption_xml(&self, video_id: &str) -> Result<Option<(String, bool)>, YoutubeError> {
        for client in ClientKind::PLAYBACK_FALLBACKS {
            let response = match self.http.post(
                *client,
                "player",
                serde_json::json!({ "videoId": video_id }),
            ) {
                Ok(response) => response,
                Err(err) => {
                    eprintln!("youtube captions via {client:?} failed for {video_id}: {err}");
                    continue;
                }
            };
            let tracks = parse_captions(&response);
            let Some(chosen) = crate::lyrics::captions::choose(&tracks) else {
                continue;
            };
            match self.http.get_text(*client, &chosen.url) {
                Ok(xml) => return Ok(Some((xml, chosen.machine))),
                Err(err) => eprintln!("caption document from {client:?} failed: {err}"),
            }
        }
        // No captions is the normal case for music, not a failure.
        Ok(None)
    }

    /// The token a page needs, and the bookkeeping that goes with asking for it.
    ///
    /// Page 1 always begins a fresh walk, so any cursor left from a previous one is
    /// dropped; a later page needs the cursor its predecessor stored, and without it
    /// the honest answer is an error rather than showing page 1 again.
    ///
    /// Kept apart from the request so the policy is testable without a network.
    fn take_cursor(&self, keyword: &str, page: u32) -> Result<Option<String>, YoutubeError> {
        let mut cursors = self
            .cursors
            .lock()
            .map_err(|_| YoutubeError::Api("搜索游标状态已损坏，请重新搜索".to_owned()))?;
        if page <= 1 {
            cursors.remove(keyword);
            return Ok(None);
        }
        match cursors.get(keyword).cloned() {
            Some(token) => Ok(Some(token)),
            None => Err(YoutubeError::Api(
                "这一页的游标已失效，请重新搜索".to_owned(),
            )),
        }
    }

    fn store_cursor(&self, keyword: &str, token: Option<String>) {
        let Some(token) = token else { return };
        if let Ok(mut cursors) = self.cursors.lock() {
            cursors.insert(keyword.to_owned(), token);
        }
    }
}

/// The caption tracks out of a `player` response.
///
/// A video with no captions has no `captions` object at all, which is the common
/// case for music and not an error.
fn parse_captions(response: &serde_json::Value) -> Vec<crate::lyrics::captions::CaptionTrack> {
    let tracks = response
        .get("captions")
        .and_then(|captions| captions.get("playerCaptionsTracklistRenderer"))
        .and_then(|renderer| renderer.get("captionTracks"))
        .and_then(serde_json::Value::as_array);
    tracks
        .into_iter()
        .flatten()
        .filter_map(|track| {
            let url = track.get("baseUrl").and_then(serde_json::Value::as_str)?;
            Some(crate::lyrics::captions::CaptionTrack {
                language: track
                    .get("languageCode")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                // `kind: "asr"` is the documented marker; the `vssId` prefix agrees
                // (`a.en` for generated against `.en` for written).
                machine: track.get("kind").and_then(serde_json::Value::as_str) == Some("asr"),
                url: url.to_owned(),
            })
        })
        .collect()
}

impl Default for Youtube {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing here touches the network: these are the cursor rules, and the live
    /// behaviour is covered by `tests/live_youtube.rs`.
    ///
    /// A player response without captions is the common case, and must read as
    /// "none" rather than as an error.
    #[test]
    fn caption_tracks_are_read_out_of_a_player_response() {
        let response = serde_json::json!({
            "captions": { "playerCaptionsTracklistRenderer": { "captionTracks": [
                { "baseUrl": "https://example.invalid/human", "languageCode": "en", "vssId": ".en" },
                { "baseUrl": "https://example.invalid/asr", "languageCode": "en",
                  "kind": "asr", "vssId": "a.en" },
                { "languageCode": "de" }
            ]}}
        });
        let tracks = super::parse_captions(&response);
        assert_eq!(tracks.len(), 2, "an entry without a URL is skipped");
        assert!(!tracks[0].machine);
        assert!(tracks[1].machine, "kind=asr is the marker");

        assert!(super::parse_captions(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn page_one_starts_a_fresh_walk() {
        let youtube = Youtube::with_api_key("unused");
        youtube
            .cursors
            .lock()
            .unwrap()
            .insert("周杰伦".to_owned(), "STALE".to_owned());

        assert_eq!(youtube.take_cursor("周杰伦", 1).unwrap(), None);
        assert!(
            !youtube.cursors.lock().unwrap().contains_key("周杰伦"),
            "a stale cursor must not survive page 1"
        );
    }

    /// Asking for a later page without a cursor is an error the user can act on,
    /// not a silent repeat of page 1.
    #[test]
    fn a_missing_cursor_is_reported_rather_than_replayed() {
        let youtube = Youtube::with_api_key("unused");
        let err = youtube.take_cursor("周杰伦", 2).unwrap_err().to_string();
        assert!(err.contains("游标"), "got {err}");
    }

    #[test]
    fn a_stored_cursor_is_handed_to_the_next_page() {
        let youtube = Youtube::with_api_key("unused");
        youtube.store_cursor("周杰伦", Some("TOKEN".to_owned()));
        assert_eq!(
            youtube.take_cursor("周杰伦", 2).unwrap().as_deref(),
            Some("TOKEN")
        );
        // Cursors are per keyword: another search must not consume it.
        assert!(youtube.take_cursor("其他", 2).is_err());
        // A response with no further pages stores nothing.
        youtube.store_cursor("其他", None);
        assert!(youtube.take_cursor("其他", 2).is_err());
    }
}

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

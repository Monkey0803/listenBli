//! Searching, and walking the results.
//!
//! YouTube paginates with an opaque `continuation` token rather than a page
//! number, and the *shape* of a continuation response differs from the first
//! page: measured on 2026-10-10,
//!
//! * first page → `contents.twoColumnSearchResultsRenderer.primaryContents.
//!   sectionListRenderer.contents[].itemSectionRenderer.contents[].videoRenderer`,
//! * every later page → `onResponseReceivedCommands[0].appendContinuationItemsAction.
//!   continuationItems[].itemSectionRenderer.contents[].videoRenderer`.
//!
//! Both carry the next token, in a section-level `continuationItemRenderer`.
//! [`parse`] walks either shape, so the caller does not have to know which page it
//! is on.

use serde_json::Value;

use super::innertube::{ClientKind, InnerTube, YoutubeError};
use crate::api::models::{Source, Track};

/// One page of results, plus the token that fetches the next one.
#[derive(Debug, Default)]
pub struct SearchPage {
    pub tracks: Vec<Track>,
    /// `None` when the search has no further pages.
    pub continuation: Option<String>,
}

/// Ask for one page: a fresh query, or the next page of a previous one.
pub fn search(
    http: &InnerTube,
    keyword: &str,
    continuation: Option<&str>,
) -> Result<SearchPage, YoutubeError> {
    // A continuation request carries only the token; repeating the query is
    // allowed but unnecessary.
    let body = match continuation {
        Some(token) => serde_json::json!({ "continuation": token }),
        None => serde_json::json!({ "query": keyword }),
    };
    let response = http.post(ClientKind::Web, "search", body)?;
    let (tracks, continuation) = parse(&response);
    if tracks.is_empty() && continuation.is_none() {
        return Err(YoutubeError::Decode(
            "搜索结果为空，可能是风控或该关键词没有视频".to_owned(),
        ));
    }
    Ok(SearchPage {
        tracks,
        continuation,
    })
}

/// Pull the videos and the next token out of either response shape.
pub fn parse(response: &Value) -> (Vec<Track>, Option<String>) {
    let mut tracks = Vec::new();
    let mut continuation = None;

    for section in sections(response) {
        // The token sits beside the results, not inside them.
        if let Some(token) = continuation_of(&section) {
            continuation = Some(token);
        }
        let items = section
            .get("itemSectionRenderer")
            .and_then(|renderer| renderer.get("contents"))
            .and_then(Value::as_array);
        for item in items.into_iter().flatten() {
            if let Some(token) = continuation_of(item) {
                continuation = Some(token);
            }
            if let Some(video) = item.get("videoRenderer") {
                if let Some(track) = video_to_track(video) {
                    tracks.push(track);
                }
            }
        }
    }

    (tracks, continuation)
}

/// The list of sections a response may hold, whichever shape it is.
fn sections(response: &Value) -> Vec<Value> {
    let first_page = response
        .get("contents")
        .and_then(|c| c.get("twoColumnSearchResultsRenderer"))
        .and_then(|c| c.get("primaryContents"))
        .and_then(|c| c.get("sectionListRenderer"))
        .and_then(|c| c.get("contents"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !first_page.is_empty() {
        return first_page;
    }

    // Later pages append their items here instead.
    response
        .get("onResponseReceivedCommands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|command| command.get("appendContinuationItemsAction"))
        .filter_map(|action| action.get("continuationItems"))
        .filter_map(Value::as_array)
        .flatten()
        .cloned()
        .collect()
}

/// The next-page token inside a section or an item, if there is one.
fn continuation_of(node: &Value) -> Option<String> {
    node.get("continuationItemRenderer")
        .and_then(|item| item.get("continuationEndpoint"))
        .and_then(|endpoint| endpoint.get("continuationCommand"))
        .and_then(|command| command.get("token"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Normalize one `videoRenderer` into a [`Track`].
///
/// A renderer without an id or a title is not usable — YouTube includes
/// promoted shelves and channel rows in the same list — so it is skipped rather
/// than turned into a broken row.
fn video_to_track(video: &Value) -> Option<Track> {
    let id = video.get("videoId").and_then(Value::as_str)?;
    let title = text_of(video.get("title"))?;
    let author = text_of(video.get("ownerText")).unwrap_or_default();
    let duration = video
        .get("lengthText")
        .and_then(|text| text.get("simpleText"))
        .and_then(Value::as_str)
        .and_then(duration_secs)
        .unwrap_or(0);

    Some(Track {
        bvid: id.to_owned(),
        source: Source::Youtube,
        aid: 0,
        // A YouTube video has no channel index; `cid` stays 0, which is also what
        // keeps the Bilibili-only lyric path from being attempted.
        cid: 0,
        title,
        author,
        duration,
        // Built from the id rather than taken from the response: the returned
        // URLs carry signing parameters that expire, while `i.ytimg.com` serves
        // this path indefinitely.
        cover: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
    })
}

/// A title or a byline, whether YouTube sent runs or a plain string.
fn text_of(node: Option<&Value>) -> Option<String> {
    let node = node?;
    if let Some(text) = node.get("simpleText").and_then(Value::as_str) {
        return Some(text.to_owned());
    }
    let runs = node.get("runs").and_then(Value::as_array)?;
    let joined: String = runs
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect();
    (!joined.is_empty()).then_some(joined)
}

/// `"5:19"` → 319, `"1:15:32"` → 4532.
///
/// Split from the right: a bare `"319"` (which YouTube uses for clips) is seconds,
/// not minutes.
pub fn duration_secs(text: &str) -> Option<u64> {
    let mut seconds = 0u64;
    let mut parts = 0u32;
    for part in text.trim().split(':').rev() {
        let value: u64 = part.trim().parse().ok()?;
        seconds += value * 60u64.pow(parts);
        parts += 1;
        if parts > 3 {
            return None;
        }
    }
    (parts > 0).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A trimmed first page: the two fields that matter, in the real nesting.
    fn first_page() -> Value {
        json!({
            "contents": { "twoColumnSearchResultsRenderer": { "primaryContents": {
                "sectionListRenderer": { "contents": [
                    { "itemSectionRenderer": { "contents": [
                        { "videoRenderer": {
                            "videoId": "DYptgVvkVLQ",
                            "title": { "runs": [{ "text": "周杰倫 Jay Chou【晴天 Sunny Day】" }] },
                            "ownerText": { "runs": [{ "text": "周杰倫 Jay Chou" }] },
                            "lengthText": { "simpleText": "5:19" }
                        }},
                        { "videoRenderer": {
                            "videoId": "3-DteAHyRnI",
                            "title": { "simpleText": "晴天 周杰伦 (歌词版)" },
                            "ownerText": { "runs": [{ "text": "某频道" }] },
                            "lengthText": { "simpleText": "1:15:32" }
                        }},
                        { "videoRenderer": { "title": { "simpleText": "no id, skipped" } }}
                    ]}},
                    { "continuationItemRenderer": { "continuationEndpoint": {
                        "continuationCommand": { "token": "PAGE_TWO_TOKEN" }
                    }}}
                ]}
            }}}
        })
    }

    /// A trimmed later page: same renderer, different road to it.
    fn later_page() -> Value {
        json!({
            "onResponseReceivedCommands": [{ "appendContinuationItemsAction": {
                "continuationItems": [
                    { "itemSectionRenderer": { "contents": [
                        { "videoRenderer": {
                            "videoId": "YJfHuATJYsQ",
                            "title": { "runs": [{ "text": "擱淺 Step Aside" }] },
                            "ownerText": { "runs": [{ "text": "周杰倫 Jay Chou" }] },
                            "lengthText": { "simpleText": "4:03" }
                        }}
                    ]}},
                    { "continuationItemRenderer": { "continuationEndpoint": {
                        "continuationCommand": { "token": "PAGE_THREE_TOKEN" }
                    }}}
                ]
            }}]
        })
    }

    #[test]
    fn both_page_shapes_yield_tracks_and_a_token() {
        let (first, token) = parse(&first_page());
        assert_eq!(first.len(), 2, "the renderer without an id is skipped");
        assert_eq!(token.as_deref(), Some("PAGE_TWO_TOKEN"));

        let (later, token) = parse(&later_page());
        assert_eq!(later.len(), 1, "a continuation is parsed the same way");
        assert_eq!(later[0].bvid, "YJfHuATJYsQ");
        assert_eq!(token.as_deref(), Some("PAGE_THREE_TOKEN"));
    }

    #[test]
    fn a_result_becomes_a_youtube_track() {
        let (tracks, _) = parse(&first_page());
        let first = &tracks[0];
        assert_eq!(first.source, Source::Youtube);
        assert_eq!(first.bvid, "DYptgVvkVLQ");
        assert_eq!(first.author, "周杰倫 Jay Chou");
        assert_eq!(first.duration, 319);
        assert_eq!(
            first.cover.as_deref(),
            Some("https://i.ytimg.com/vi/DYptgVvkVLQ/hqdefault.jpg"),
            "the id builds the cover, not the expiring URL from the response"
        );
        // A YouTube track must not be treated as a Bilibili one downstream.
        assert_eq!(first.cid, 0);
        assert_eq!(first.key(), "yt:DYptgVvkVLQ");
        assert_eq!(first.cache_key(), "DYptgVvkVLQ");

        // The second used `simpleText` instead of runs, and a long duration.
        assert_eq!(tracks[1].title, "晴天 周杰伦 (歌词版)");
        assert_eq!(tracks[1].duration, 4532);
    }

    #[test]
    fn durations_are_read_from_the_right() {
        assert_eq!(duration_secs("5:19"), Some(319));
        assert_eq!(duration_secs("1:15:32"), Some(4532));
        assert_eq!(duration_secs("319"), Some(319), "a bare number is seconds");
        assert_eq!(duration_secs("0:07"), Some(7));
        assert_eq!(duration_secs(""), None);
        assert_eq!(duration_secs("live"), None);
    }

    #[test]
    fn an_empty_or_unknown_response_is_empty_not_a_panic() {
        assert_eq!(parse(&json!({})).0.len(), 0);
        assert!(parse(&json!({ "contents": {} })).1.is_none());
        // Renderers missing their title are dropped rather than shown blank.
        let (tracks, _) = parse(&json!({
            "contents": { "twoColumnSearchResultsRenderer": { "primaryContents": {
                "sectionListRenderer": { "contents": [{ "itemSectionRenderer": { "contents": [
                    { "videoRenderer": { "videoId": "x" } }
                ]}}]}
            }}}
        }));
        assert!(tracks.is_empty());
    }
}

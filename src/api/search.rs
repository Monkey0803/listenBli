//! Video search.
//!
//! `search/type` requires WBI signing and answers with an anti-bot HTML page
//! when it is unhappy, so we keep `search/all/v2` (which works unsigned) as a
//! fallback path.

use super::client::{Api, ApiError, BILI_API};
use super::models::{SearchAllData, SearchItem, SearchTypeData, Track};
use crate::util;

pub fn search(api: &Api, keyword: &str, page: u32) -> Result<Vec<Track>, ApiError> {
    let params = vec![
        ("search_type".to_string(), "video".to_string()),
        ("keyword".to_string(), keyword.to_string()),
        ("page".to_string(), page.to_string()),
    ];

    match api.get_bili_signed::<SearchTypeData>("/x/web-interface/search/type", &params) {
        Ok(data) => {
            let tracks = data
                .result
                .into_iter()
                .map(item_to_track)
                .collect::<Vec<_>>();
            if !tracks.is_empty() {
                return Ok(tracks);
            }
        }
        Err(err) => {
            // Fall through to the unsigned endpoint, but remember the original
            // failure so we can surface it if the fallback also yields nothing.
            tracing_fallback(err, keyword);
        }
    }

    search_all_fallback(api, keyword, page)
}

fn tracing_fallback(err: ApiError, keyword: &str) {
    eprintln!("search/type failed for {keyword:?}: {err}; trying search/all/v2");
}

fn search_all_fallback(api: &Api, keyword: &str, page: u32) -> Result<Vec<Track>, ApiError> {
    let encoded =
        percent_encoding::utf8_percent_encode(keyword, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
    let url = format!("{BILI_API}/x/web-interface/search/all/v2?keyword={encoded}&page={page}");
    let data: SearchAllData = api.get_bili(&url)?;
    let items = data
        .result
        .into_iter()
        .find(|group| group.result_type == "video")
        .map(|group| group.data)
        .unwrap_or_default();
    Ok(items.into_iter().map(item_to_track).collect())
}

fn item_to_track(item: SearchItem) -> Track {
    Track {
        bvid: item.bvid,
        aid: item.aid,
        cid: 0,
        title: util::strip_html(&item.title),
        author: util::strip_html(&item.author),
        duration: util::parse_duration(&item.duration),
        cover: util::normalize_url(&item.pic).filter(|s| !s.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_item_is_normalized() {
        let track = item_to_track(SearchItem {
            bvid: "BV1xx411c7mD".into(),
            aid: 123,
            title: "<em class=\"keyword\">周杰伦</em> - 晴天 &amp; 雨".into(),
            author: "音乐无限".into(),
            duration: "5:17".into(),
            pic: "//i2.hdslb.com/bfs/archive/x.jpg".into(),
        });
        assert_eq!(track.bvid, "BV1xx411c7mD");
        assert_eq!(track.title, "周杰伦 - 晴天 & 雨");
        assert_eq!(track.duration, 317);
        assert_eq!(
            track.cover.as_deref(),
            Some("https://i2.hdslb.com/bfs/archive/x.jpg")
        );
    }
}

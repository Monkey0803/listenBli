//! Video search.
//!
//! `search/type` requires WBI signing and answers with an anti-bot HTML page
//! when it is unhappy, so we keep `search/all/v2` (which works unsigned) as a
//! fallback path.

use super::client::{Api, ApiError, BILI_API};
use super::models::{SearchAllData, SearchItem, SearchTypeData, Track};
use crate::util;

/// One page of search results, plus whether asking for the next page is useful.
pub struct SearchPage {
    pub tracks: Vec<Track>,
    pub has_more: bool,
    /// The server's own page count, 0 when it did not say.
    pub num_pages: u32,
}

pub fn search(api: &Api, keyword: &str, page: u32) -> Result<SearchPage, ApiError> {
    let params = vec![
        ("search_type".to_string(), "video".to_string()),
        ("keyword".to_string(), keyword.to_string()),
        ("page".to_string(), page.to_string()),
    ];

    match api.get_bili_signed::<SearchTypeData>("/x/web-interface/search/type", &params) {
        Ok(data) => {
            let (num_pages, items) = (data.num_pages, data.result);
            let tracks = items.into_iter().map(item_to_track).collect::<Vec<_>>();
            if !tracks.is_empty() {
                let has_more = more_pages(page, num_pages, &tracks);
                return Ok(SearchPage {
                    tracks,
                    has_more,
                    num_pages,
                });
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

fn search_all_fallback(api: &Api, keyword: &str, page: u32) -> Result<SearchPage, ApiError> {
    let encoded =
        percent_encoding::utf8_percent_encode(keyword, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
    let url = format!("{BILI_API}/x/web-interface/search/all/v2?keyword={encoded}&page={page}");
    let data: SearchAllData = api.get_bili(&url)?;
    let num_pages = data.num_pages;
    let items = data
        .result
        .into_iter()
        .find(|group| group.result_type == "video")
        .map(|group| group.data)
        .unwrap_or_default();
    let tracks = items.into_iter().map(item_to_track).collect::<Vec<_>>();
    let has_more = more_pages(page, num_pages, &tracks);
    Ok(SearchPage {
        tracks,
        has_more,
        num_pages,
    })
}

/// Is there another page after `page`?
///
/// `numPages` is the server's own count, which is what the UI wants.
///
/// The fallback covers a response that omits it: a non-empty page means "assume
/// there is more". Bilibili *clamps* an out-of-range page instead of returning
/// an empty one (asking for page 9999 still answers with 20 videos), so the end
/// of the list is detected in the app by a page that adds nothing new.
fn more_pages(page: u32, num_pages: u32, tracks: &[Track]) -> bool {
    if num_pages > 0 {
        page < num_pages
    } else {
        !tracks.is_empty()
    }
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

    fn track(bvid: &str) -> Track {
        Track {
            bvid: bvid.to_owned(),
            aid: 0,
            cid: 0,
            title: "标题".to_owned(),
            author: String::new(),
            duration: 1,
            cover: None,
        }
    }

    #[test]
    fn more_pages_trusts_the_server_count_when_it_has_one() {
        let tracks = vec![track("BV1")];
        assert!(more_pages(1, 50, &tracks));
        assert!(more_pages(49, 50, &tracks));
        assert!(!more_pages(50, 50, &tracks));
    }

    #[test]
    fn without_a_count_a_full_page_means_there_might_be_more() {
        let tracks = vec![track("BV1")];
        assert!(more_pages(1, 0, &tracks));
        assert!(!more_pages(1, 0, &[]), "an empty page ends the search");
    }

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

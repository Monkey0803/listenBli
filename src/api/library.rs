//! "My" data that requires a login: favourites and watch history.
//!
//! The exact response shape of these endpoints could not be exercised without a
//! real account, so every field is optional and malformed rows are skipped
//! rather than treated as fatal.

use super::client::{Api, ApiError};
use super::models::{FavFolder, FavFolderListData, FavResourceListData, HistoryData, Track};
use crate::util;

pub fn fav_folders(api: &Api, mid: i64) -> Result<Vec<FavFolder>, ApiError> {
    let params = vec![("up_mid".to_string(), mid.to_string())];
    let data: FavFolderListData =
        api.get_bili_signed("/x/v3/fav/folder/created/list-all", &params)?;
    Ok(data.list)
}

/// Returns the page's items and whether more pages exist.
pub fn fav_items(api: &Api, media_id: i64, page: u32) -> Result<(Vec<Track>, bool), ApiError> {
    let params = vec![
        ("media_id".to_string(), media_id.to_string()),
        ("pn".to_string(), page.to_string()),
        ("ps".to_string(), "20".to_string()),
        ("platform".to_string(), "web".to_string()),
    ];
    let data: FavResourceListData = api.get_bili_signed("/x/v3/fav/resource/list", &params)?;

    let tracks = data
        .medias
        .unwrap_or_default()
        .into_iter()
        // attr 1 = single video. Audio collections (2/12) and deleted items
        // (attr 9) have no playable DASH stream.
        .filter(|m| m.attr == 1 && !m.bvid.is_empty())
        .map(|m| Track {
            bvid: m.bvid,
            aid: m.id,
            cid: 0,
            title: util::strip_html(&m.title),
            author: m.upper.map(|u| u.name).unwrap_or_default(),
            duration: m.duration,
            cover: util::normalize_url(&m.cover),
        })
        .collect();

    Ok((tracks, data.has_more))
}

pub fn history(api: &Api, max: i64, view_at: i64) -> Result<Vec<Track>, ApiError> {
    let params = vec![
        ("ps".to_string(), "30".to_string()),
        ("max".to_string(), max.to_string()),
        ("view_at".to_string(), view_at.to_string()),
    ];
    let data: HistoryData = api.get_bili_signed("/x/web-interface/history/cursor", &params)?;

    Ok(data
        .list
        .into_iter()
        .filter_map(|item| {
            let refer = item.history?;
            if refer.bvid.is_empty() {
                // Live rooms, articles and PGC entries have no bvid.
                return None;
            }
            Some(Track {
                bvid: refer.bvid,
                aid: refer.oid,
                cid: refer.cid,
                title: util::strip_html(&item.title),
                author: item.author_name,
                duration: item.duration,
                cover: util::normalize_url(&item.cover),
            })
        })
        .collect())
}

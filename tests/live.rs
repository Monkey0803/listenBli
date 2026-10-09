//! Integration tests that talk to the real Bilibili and NetEase endpoints.
//!
//! They are `#[ignore]`d so `cargo test` stays offline and deterministic. Run
//! them deliberately with:
//!
//! ```text
//! cargo test --test live -- --ignored --nocapture
//! ```

use listenbli::api::client::{Api, BILI_API};
use listenbli::api::cookie::CookieJar;
use listenbli::api::models::SearchTypeData;
use listenbli::api::wbi::{self, now_ts};
use listenbli::api::{search, video};
use listenbli::lyrics::{self, LyricsSource};

/// A well-known music video with a stable id.
const BVID: &str = "BV1GJ411x7h7";

fn api() -> Api {
    let api = Api::new(CookieJar::default());
    // Anonymous fingerprint; failure here is not fatal for these tests.
    let _ = api.bootstrap_fingerprint();
    api
}

/// Validates the WBI implementation end to end *without* the unsigned fallback
/// that `get_bili_signed` would otherwise mask a bug with.
///
/// This is the test that proves the percent-encoding choice (space as `%20`)
/// matches what Bilibili's server recomputes.
#[test]
#[ignore = "hits the live Bilibili API"]
fn wbi_signed_request_is_accepted() {
    let api = api();
    let mixin = api.mixin_key().expect("nav should expose wbi keys");

    let params = vec![
        ("search_type".to_string(), "video".to_string()),
        ("keyword".to_string(), "周杰伦 晴天".to_string()),
        ("page".to_string(), "1".to_string()),
    ];
    let query = wbi::sign_query(&params, &mixin, now_ts());
    let url = format!("{BILI_API}/x/web-interface/search/type?{query}");

    let data: SearchTypeData = api
        .get_bili(&url)
        .expect("a correctly signed request must be accepted (got risk control?)");

    assert!(!data.result.is_empty(), "expected search hits");
    println!(
        "signed search returned {} results; first = {}",
        data.result.len(),
        data.result[0].title
    );
}

#[test]
#[ignore = "hits the live Bilibili API"]
fn search_returns_normalized_tracks() {
    let api = api();
    let page = search::search(&api, "周杰伦 晴天", 1).expect("search should succeed");
    let tracks = page.tracks;
    assert!(!tracks.is_empty(), "expected results");
    assert!(page.has_more, "a broad query should have a second page");

    let first = &tracks[0];
    assert!(
        first.bvid.starts_with("BV"),
        "bvid looks wrong: {}",
        first.bvid
    );
    assert!(
        !first.title.contains('<'),
        "html tags must be stripped: {}",
        first.title
    );
    assert!(
        !first.title.contains("<em"),
        "keyword markup must be stripped"
    );
    assert!(first.duration > 0, "duration should have been parsed");
    assert!(
        first
            .cover
            .as_deref()
            .is_some_and(|c| c.starts_with("https://")),
        "cover should be absolute: {:?}",
        first.cover
    );
    println!("first result: {} ({})", first.title, first.bvid);
}

/// The UI's "加载更多" relies on `page` really changing the result set.
#[test]
#[ignore = "hits the live Bilibili API"]
fn search_pages_return_different_results() {
    let api = api();
    let first = search::search(&api, "周杰伦", 1).expect("page 1 should succeed");
    assert!(first.has_more, "expected a second page for a broad query");

    let second = search::search(&api, "周杰伦", 2).expect("page 2 should succeed");
    assert!(!second.tracks.is_empty(), "page 2 should have results");

    let first_ids: std::collections::HashSet<&str> =
        first.tracks.iter().map(|t| t.bvid.as_str()).collect();
    let overlap = second
        .tracks
        .iter()
        .filter(|t| first_ids.contains(t.bvid.as_str()))
        .count();
    let fresh = second.tracks.len() - overlap;
    println!(
        "page 1 = {} results, page 2 = {} results, {overlap} repeated, {fresh} new",
        first.tracks.len(),
        second.tracks.len()
    );
    // B 站的分页排序不稳定：实测第 2 页会重复第 1 页里的 4 首。App 在追加时按
    // bvid 去重，所以这里只要求第 2 页确实带来了新结果。
    assert!(
        fresh >= second.tracks.len() / 2,
        "page 2 should mostly be new results, only {fresh} of {} were",
        second.tracks.len()
    );

    assert!(
        first.num_pages > 1,
        "the server should report its page count, got {}",
        first.num_pages
    );

    // Far past the end: Bilibili has answered both ways in practice — a clamped
    // page that repeats the tail (still claiming `has_more`) and a plain error.
    // Either is fine, because the UI stops at `has_more == false` and also stops
    // when a page adds nothing new; what must never happen is offering another
    // page that has content.
    match search::search(&api, "周杰伦", 9999) {
        Ok(page) => {
            println!(
                "page 9999 = {} results, has_more = {} (clamped, not empty)",
                page.tracks.len(),
                page.has_more
            );
            assert!(!page.has_more, "no page beyond the end may offer another");
        }
        Err(err) => println!("page 9999 直接报错（{err}），UI 在 has_more=false 时本就不会请求它"),
    }
}

#[test]
#[ignore = "hits the live Bilibili API"]
fn playurl_offers_a_decodable_aac_lc_stream() {
    let api = api();
    let mut track = listenbli::api::models::Track {
        bvid: BVID.to_string(),
        aid: 0,
        cid: 0,
        title: String::new(),
        author: String::new(),
        duration: 0,
        cover: None,
    };
    video::resolve_track(&api, &mut track).expect("view lookup should succeed");
    assert_ne!(track.cid, 0, "cid must be resolved");
    assert!(track.duration > 0, "duration must be resolved");
    assert!(!track.title.is_empty(), "title must be resolved");

    let playurl = video::playurl(&api, BVID, track.cid).expect("playurl should succeed");
    println!("available streams: {}", video::describe_streams(&playurl));

    let picked = video::pick_audio(&playurl, false).expect("a stream must be pickable");
    assert_eq!(
        picked.quality.label(),
        "192K",
        "anonymous requests should get 192K AAC-LC"
    );
    assert!(!picked.primary().is_empty());

    // The URL must actually serve an ISO-BMFF payload with the desktop referer.
    let bytes = api
        .get_bytes(picked.primary())
        .expect("CDN download should succeed");
    assert!(
        listenbli::audio::looks_like_iso_bmff(&bytes),
        "CDN did not return ISO-BMFF ({} bytes)",
        bytes.len()
    );
    println!("downloaded {} bytes from the picked stream", bytes.len());
}

/// The whole lyrics pipeline: video with no CC subtitle -> NetEase fallback.
#[test]
#[ignore = "hits the live Bilibili and NetEase APIs"]
fn lyrics_fall_back_to_netease() {
    let api = api();
    let mut track = listenbli::api::models::Track {
        bvid: BVID.to_string(),
        aid: 0,
        cid: 0,
        title: String::new(),
        author: String::new(),
        duration: 0,
        cover: None,
    };
    video::resolve_track(&api, &mut track).expect("view lookup should succeed");

    let lyrics = lyrics::fetch_for(&api, &track);
    println!(
        "lyrics source = {:?}, {} lines",
        lyrics.source,
        lyrics.lines.len()
    );
    if !lyrics.is_empty() {
        for line in lyrics.lines.iter().take(3) {
            println!("  {:>8.2}s  {}", line.time.as_secs_f64(), line.text);
        }
    }

    assert!(!lyrics.is_empty(), "expected NetEase to supply lyrics");
    assert_eq!(lyrics.source, LyricsSource::Netease);
}

/// The realistic Chinese case: the uploader's title carries decorations and the
/// artist, while NetEase's track name is just the song. This exercises the
/// containment-aware title matching.
#[test]
#[ignore = "hits the live Bilibili and NetEase APIs"]
fn lyrics_match_a_chinese_song_whose_title_carries_the_artist() {
    let api = api();
    let tracks = search::search(&api, "周杰伦 晴天", 1)
        .expect("search should succeed")
        .tracks;
    let mut track = tracks
        .into_iter()
        .find(|t| t.title.contains("晴天"))
        .expect("expected a result whose title contains the song name");

    println!(
        "bilibili title   = {:?}\nnetease query     = {:?}",
        track.title,
        listenbli::lyrics::netease::build_query(&track.title)
    );

    video::resolve_track(&api, &mut track).expect("view lookup should succeed");
    let lyrics = lyrics::fetch_for(&api, &track);

    println!(
        "lyrics source = {:?}, {} lines",
        lyrics.source,
        lyrics.lines.len()
    );
    for line in lyrics.lines.iter().take(4) {
        println!("  {:>8.2}s  {}", line.time.as_secs_f64(), line.text);
    }

    assert_eq!(
        lyrics.source,
        LyricsSource::Netease,
        "expected a third-party lyric match for {:?}",
        track.title
    );
    assert!(!lyrics.is_empty());
}

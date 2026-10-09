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
    let tracks = search::search(&api, "周杰伦 晴天", 1).expect("search should succeed");
    assert!(!tracks.is_empty(), "expected results");

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
    let tracks = search::search(&api, "周杰伦 晴天", 1).expect("search should succeed");
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

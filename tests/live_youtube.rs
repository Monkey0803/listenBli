//! Live tests for the YouTube source.
//!
//! Ignored by default: they hit the real service, which is rate-limited, may be
//! blocked, and can change shape without notice. Run them when touching the
//! YouTube client:
//!
//! ```text
//! cargo test --test live_youtube -- --ignored --nocapture
//! ```
//!
//! Read them as documentation of what "working" means, not as a regression suite
//! someone else's service can be expected to keep green.

use listenbli::api::youtube::{ClientKind, InnerTube, Youtube};

/// A video that has been publicly available for many years.
const VIDEO: &str = "dQw4w9WgXcQ";

#[test]
#[ignore = "hits the live YouTube service"]
fn search_returns_tracks_and_a_second_page() {
    let youtube = Youtube::new();

    let (first, has_more) = youtube.search("周杰伦 晴天", 1).expect("a first page");
    println!("page 1: {} tracks, has_more={has_more}", first.len());
    assert!(first.len() >= 10, "a broad query should fill a page");
    assert!(has_more, "a broad query should have more pages");

    // Every row has to be usable: an id to resolve, a title to show, a length to
    // draw. A live stream has no length, so it is only required of the rest.
    for track in &first {
        assert!(!track.bvid.is_empty(), "no video id: {track:?}");
        assert!(!track.title.is_empty(), "no title: {track:?}");
        assert_eq!(track.source, listenbli::api::Source::Youtube);
        assert!(
            track.cache_key() == track.bvid,
            "a YouTube track is cached under its video id"
        );
    }
    println!("first: {} — {}s", first[0].title, first[0].duration);
    assert!(first.iter().any(|track| track.duration > 0), "no durations");

    let (second, _) = youtube.search("周杰伦 晴天", 2).expect("a second page");
    println!("page 2: {} tracks", second.len());
    assert!(!second.is_empty(), "the second page should have results");

    // Pages must not repeat each other: the app's list dedupes by key, and a
    // paginated walk that returns page 1 again would stall it.
    let first_ids: Vec<&str> = first.iter().map(|track| track.bvid.as_str()).collect();
    let repeats = second
        .iter()
        .filter(|track| first_ids.contains(&track.bvid.as_str()))
        .count();
    assert!(repeats < second.len(), "page 2 repeated everything");
}

/// The container claim that lets the whole playback path be reused: a resolved
/// YouTube audio stream is a fragmented MP4 that starts `ftyp` + `moov` + `sidx`,
/// exactly like Bilibili's DASH audio.
#[test]
#[ignore = "hits the live YouTube service"]
fn a_resolved_stream_is_a_fragmented_mp4_in_reach() {
    let youtube = Youtube::new();
    let playable = youtube.resolve(VIDEO).expect("a playable video");
    println!(
        "{} — {} ({}s, live={})",
        playable.title, playable.author, playable.duration, playable.is_live
    );
    assert!(!playable.is_live);
    assert!(!playable.picks.is_empty(), "no decodable stream");

    let pick = playable.best_audio().expect("an AAC stream");
    println!(
        "picked itag {} ({:?}), {} bytes",
        pick.tag,
        pick.quality,
        pick.content_length.unwrap_or(0)
    );
    assert_eq!(pick.tag, 140, "AAC-LC is the one to want");

    // Fetch only the head: the point is the container, not the audio.
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("http client");
    let response = client
        .get(&pick.url)
        .header("Range", "bytes=0-1023")
        .header("Accept-Encoding", "identity")
        .header(
            "User-Agent",
            "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12; GB) gzip",
        )
        .send()
        .expect("a ranged request");

    // A ranged request is also the only fast one: an un-ranged sequential GET is
    // throttled to a crawl (measured at ~0.03 MB/s), which is why the downloader
    // asks in chunks.
    assert_eq!(
        response.status().as_u16(),
        206,
        "expected a partial response"
    );
    let total = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit('/').next())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("Content-Range carries the real total");
    assert!(total > 100_000, "suspiciously small stream: {total}");
    if let Some(advertised) = pick.content_length {
        assert_eq!(
            advertised, total,
            "the advertised length must be the real one"
        );
    }

    let head = response.bytes().expect("the head bytes");
    let mut at = 0usize;
    let mut boxes = Vec::new();
    while at + 8 <= head.len() && boxes.len() < 4 {
        let size = u32::from_be_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        boxes.push(String::from_utf8_lossy(&head[at + 4..at + 8]).into_owned());
        if size < 8 {
            break;
        }
        at += size as usize;
    }
    println!("head boxes: {boxes:?}");
    assert_eq!(
        boxes,
        ["ftyp", "moov", "sidx"],
        "the streaming reader and the export path assume this shape"
    );
}

/// A refusal must reach the user in YouTube's own words.
#[test]
#[ignore = "hits the live YouTube service"]
fn an_unplayable_video_reports_youtubes_reason() {
    let youtube = Youtube::new();
    // A video that is deliberately not embeddable/available.
    match youtube.resolve("aaaaaaaaaaa") {
        Ok(playable) => panic!("a made-up id should not resolve: {playable:?}"),
        Err(err) => {
            let text = err.to_string();
            println!("refusal: {text}");
            assert!(!text.is_empty());
        }
    }
}

/// The web client is refused playback; document that so the choice of
/// `ANDROID_VR` is not mistaken for an arbitrary one.
#[test]
#[ignore = "hits the live YouTube service"]
fn the_web_client_is_refused_playback() {
    let http = InnerTube::new();
    let outcome = listenbli::api::youtube::player::resolve(&http, ClientKind::Web, VIDEO);
    match outcome {
        Ok(_) => println!("WEB now plays again — the client list may be simplified"),
        Err(err) => {
            println!("WEB playback refused as expected: {err}");
            assert!(
                err.to_string().contains("无法播放"),
                "expected a playback refusal, got {err}"
            );
        }
    }
}

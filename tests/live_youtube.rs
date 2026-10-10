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

/// YouTube's own subtitles: the caption list, the chosen track, and the parse.
#[test]
#[ignore = "hits the live YouTube service"]
fn youtubes_own_captions_parse_into_timed_lines() {
    use listenbli::api::models::{Source, Track};
    use listenbli::lyrics::captions;

    let youtube = Youtube::new();

    let tracks = youtube.captions(VIDEO).expect("a caption list");
    let listed: Vec<(&str, bool)> = tracks
        .iter()
        .map(|track| (track.language.as_str(), track.machine))
        .collect();
    println!("{} caption tracks: {listed:?}", tracks.len());
    assert!(!tracks.is_empty(), "this video has captions");
    assert!(
        tracks.iter().any(|track| !track.machine),
        "and at least one written by a person"
    );

    // The document is bound to the client that named it, so the client fetches it:
    // with a browser user agent the same URL returns an empty body.
    let (xml, machine) = youtube
        .caption_xml(VIDEO)
        .expect("the caption document")
        .expect("a document");
    let lines = captions::parse_timedtext(&xml);
    println!(
        "{} lines (machine={machine}), first {:?}",
        lines.len(),
        lines.first()
    );
    assert!(lines.len() > 10, "a real document: {}", lines.len());
    assert!(
        lines.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "timestamps must ascend"
    );

    // And through the provider, which is what the lyrics worker calls.
    let track = Track {
        bvid: VIDEO.to_owned(),
        source: Source::Youtube,
        aid: 0,
        cid: 0,
        title: "Never Gonna Give You Up".to_owned(),
        author: "Rick Astley".to_owned(),
        duration: 213,
        cover: None,
    };
    let (lyrics, machine) =
        captions::fetch_for(&youtube, &track).expect("the provider should find a document");
    println!(
        "provider: {} lines, machine={machine}, source={:?}",
        lyrics.lines.len(),
        lyrics.source
    );
    assert!(!machine, "an official music video has human subtitles");
    assert!(lyrics.lines.len() > 10);
}

/// LRCLib is the one lyric provider that serves both platforms, which is exactly
/// why it was worth adding when YouTube turned out to have no usable native source.
#[test]
#[ignore = "hits the live LRCLib service"]
fn lrclib_serves_a_track_from_either_platform() {
    use listenbli::api::client::Api;
    use listenbli::api::cookie::CookieJar;
    use listenbli::api::models::{Source, Track};

    let api = Api::new(CookieJar::default());
    // The same song, once as each platform would describe it.
    for source in [Source::Youtube, Source::Bilibili] {
        let track = Track {
            bvid: "dQw4w9WgXcQ".to_owned(),
            source,
            aid: 0,
            cid: 0,
            title: "Never Gonna Give You Up".to_owned(),
            author: "Rick Astley".to_owned(),
            duration: 213,
            cover: None,
        };
        let lyrics = listenbli::lyrics::lrclib::fetch(&api, &track)
            .expect("the request should succeed")
            .expect("a synced document");
        println!(
            "{source:?}: {} lines from {:?}, first {:?}",
            lyrics.lines.len(),
            lyrics.source,
            lyrics.lines.first().map(|line| (&line.text, line.time))
        );
        assert!(
            lyrics.lines.len() > 5,
            "a real document: {}",
            lyrics.lines.len()
        );
        assert!(
            lyrics
                .lines
                .windows(2)
                .all(|pair| pair[0].time <= pair[1].time),
            "timestamps must ascend"
        );
    }
}

/// The case that broke on a real desktop: a long music compilation that the
/// headset client answers with "Sign in to confirm you're not a bot".
///
/// The chain has to fall through to another client, so this asserts on the chain —
/// what any single client does is logged, not asserted, because the gate moves
/// between videos and over time.
#[test]
#[ignore = "hits the live YouTube service"]
fn a_gated_music_compilation_still_resolves() {
    use listenbli::api::youtube::{ClientKind, InnerTube};

    // Measured: the headset client refuses these with LOGIN_REQUIRED, the phone
    // client answers with two AAC streams.
    const GATED: &str = "9mplI5qEhxk";

    let http = InnerTube::new();
    match listenbli::api::youtube::player::resolve(&http, ClientKind::AndroidVr, GATED) {
        Ok(playable) => println!(
            "note: AndroidVr now answers for this video ({} picks) — the gate moved",
            playable.picks.len()
        ),
        Err(err) => println!("AndroidVr refused as expected: {err}"),
    }

    let youtube = Youtube::new();
    let playable = youtube
        .resolve(GATED)
        .expect("the fallback chain should resolve a gated video");
    println!(
        "resolved via the chain: {} — {}s, {} decodable stream(s)",
        playable.title,
        playable.duration,
        playable.picks.len()
    );
    assert!(!playable.title.is_empty());
    assert!(
        playable.duration > 600,
        "a long compilation: {}",
        playable.duration
    );
    assert!(
        playable.best_audio().is_some(),
        "the fallback must yield something the decoder can open"
    );
}

/// Searching YouTube through the worker, exactly as the UI does it.
///
/// This is the milestone-5 wiring check: the app sends one command with a platform
/// on it, and the worker decides which client to ask.
#[test]
#[ignore = "hits the live YouTube service"]
fn the_worker_routes_a_youtube_search() {
    use std::sync::Arc;
    use std::time::Duration;

    use listenbli::api::client::Api;
    use listenbli::api::cookie::CookieJar;
    use listenbli::api::Source;
    use listenbli::config::{self, Config};
    use listenbli::net::{self, Cmd, Evt};

    let api = Arc::new(Api::new(CookieJar::default()));
    let worker = net::spawn(Arc::clone(&api), config::shared(Config::default()));
    worker
        .cmd_tx
        .send(Cmd::Search {
            source: Source::Youtube,
            keyword: "周杰伦 晴天".to_owned(),
            page: 1,
        })
        .expect("the worker should accept the search");

    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        assert!(std::time::Instant::now() < deadline, "timed out");
        match worker.evt_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Evt::SearchResults {
                tracks, has_more, ..
            }) => {
                println!(
                    "worker returned {} YouTube tracks, has_more={has_more}",
                    tracks.len()
                );
                assert!(tracks.len() >= 10, "a broad query should fill a page");
                assert!(has_more, "a broad query should have more pages");
                assert!(
                    tracks.iter().all(|track| track.source == Source::Youtube),
                    "every row must be tagged as YouTube"
                );
                assert!(
                    tracks.iter().all(|track| !track.title.is_empty()),
                    "every row needs a title"
                );
                break;
            }
            Ok(Evt::Error { context, message }) => panic!("{context}: {message}"),
            Ok(_) => {}
            Err(err) => panic!("waiting for results failed: {err}"),
        }
    }
}

/// Downloading a YouTube track through the real worker: the bytes land in the
/// cache, the fragment is converted to a playable `.m4a`, and it happens at the
/// speed ranges allow rather than at the speed a plain GET is throttled to.
///
/// The timing assertion is the point of this test. A plain sequential GET measured
/// ~0.03 MB/s (3.15 MB took 97 s), so this file would take minutes without the
/// range plan — and the failure mode of "someone removed the Range header" is a
/// hang, not an error, which nothing else would catch.
#[test]
#[ignore = "hits the live YouTube service and writes to a temp cache"]
fn a_youtube_track_downloads_into_the_cache_and_becomes_playable() {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use listenbli::api::client::Api;
    use listenbli::api::cookie::CookieJar;
    use listenbli::api::models::{Source, Track};
    use listenbli::config::{self, Config};
    use listenbli::net::{self, Cmd, Evt};

    // A cache under the test's own directory: `HOME` decides where it lands.
    let home = std::env::temp_dir().join(format!("listenbli-yt-live-{}", std::process::id()));
    std::env::set_var("HOME", &home);
    let _ = std::fs::create_dir_all(&home);

    let api = Arc::new(Api::new(CookieJar::default()));
    let worker = net::spawn(Arc::clone(&api), config::shared(Config::default()));
    let cache = Arc::clone(&worker.cache);

    let track = Track {
        bvid: VIDEO.to_owned(),
        source: Source::Youtube,
        aid: 0,
        cid: 0,
        title: "live download".to_owned(),
        author: String::new(),
        duration: 0,
        cover: None,
    };
    let start = Instant::now();
    worker
        .cmd_tx
        .send(Cmd::LoadTrack(Box::new(track)))
        .expect("the worker should accept the job");

    // The worker resolves, downloads, converts, and only then announces playback.
    let mut ready = None;
    while start.elapsed() < Duration::from_secs(120) {
        match worker.evt_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Evt::TrackReady { track, source, .. }) => {
                ready = Some((track, source));
                break;
            }
            Ok(Evt::Error { context, message }) => panic!("{context}: {message}"),
            Ok(_) => {}
            Err(err) => panic!("timed out waiting for playback: {err}"),
        }
    }
    let (track, source) = ready.expect("playback should have started");
    let elapsed = start.elapsed();

    println!(
        "ready in {:.1}s: {} — {}s",
        elapsed.as_secs_f32(),
        track.title,
        track.duration
    );
    assert!(!track.title.is_empty(), "the metadata should be filled in");
    assert!(track.duration > 100, "a real duration: {}", track.duration);
    println!("source: {source:?}");

    // The cache now holds a playable file, not a fragment: the download converts
    // it once the bytes are whole.
    let ready_path = cache.ready_path_for(&track.cache_key(), 140);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready_path.is_file() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        ready_path.is_file(),
        "expected a playable .m4a at {ready_path:?}"
    );
    let size = std::fs::metadata(&ready_path).expect("metadata").len();
    println!("cache: {} ({size} bytes)", ready_path.display());
    assert!(size > 1_000_000, "suspiciously small: {size}");

    // Ranges made this fast; an un-ranged GET of this file takes minutes.
    assert!(
        elapsed < Duration::from_secs(45),
        "the download took {elapsed:?}; the range plan is probably not in use"
    );

    let _ = std::fs::remove_dir_all(&home);
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

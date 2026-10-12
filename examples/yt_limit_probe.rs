//! Diagnostic: how much of a YouTube audio stream is reachable, and by which client?
//!
//! YouTube serves a signed URL that carries only the *opening* of a large file to
//! clients that present no PoToken. This probe measures that opening rather than
//! describing it, and checks every escape route that looked plausible:
//!
//! * asking for a range whose end is past the opening (refused — 403),
//! * asking for a range that *starts* inside it and ends past it (refused),
//! * asking with no end at all (`bytes=0-`) (refused),
//! * putting the range in the URL instead of the header (`&range=` is not signed;
//!   the CDN answers 416),
//! * re-resolving for a fresh signature (the new URL has its own, no larger, opening),
//! * a manifest route (`hlsManifestUrl`/`dashManifestUrl` — not offered at all),
//! * another client identity (`--clients`; every one either refuses the video or is
//!   capped the same way).
//!
//! Measured 2026-10-12, on four videos of 22 MB to 591 MB: the last readable byte was
//! 1,071,03x–1,071,067 every time, and a 3.4 MB video was readable end to end. So the
//! limit is real, external, and not something this app can lift.
//!
//! ```text
//! cargo run --example yt_limit_probe -- <keyword | videoId>
//! cargo run --example yt_limit_probe -- --clients <videoId>
//! ```

use listenbli::api::client::Api;
use listenbli::api::cookie::CookieJar;
use listenbli::api::youtube::innertube::{ClientKind, InnerTube};
use listenbli::api::youtube::player;

const KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--clients") {
        let video = args.get(1).cloned().unwrap_or_else(|| "dQw4w9WgXcQ".into());
        clients(&video);
        return;
    }
    let arg = args
        .first()
        .cloned()
        .unwrap_or_else(|| "dQw4w9WgXcQ".into());
    if arg.len() != 11 {
        search(&arg);
        return;
    }
    ranges(&arg);
}

fn search(keyword: &str) {
    let youtube = listenbli::api::youtube::Youtube::new();
    match youtube.search(keyword, 1) {
        Ok((tracks, _)) => {
            for track in tracks {
                // The video id in the `bvid` field is what the other modes want.
                println!("{} {}s {}", track.bvid, track.duration, track.title);
            }
        }
        Err(err) => println!("search failed: {err}"),
    }
}

/// One ranged GET, reported as its status or as the reason it failed.
fn ask(api: &Api, url: &str, start: u64, end: u64) -> String {
    match api.get_stream_range(url, &format!("bytes={start}-{end}")) {
        Ok(response) => format!("{}", response.status()),
        Err(err) => format!("{err}"),
    }
}

fn ranges(video: &str) {
    let http = InnerTube::new();
    let api = Api::new(CookieJar::default());

    for client in ClientKind::PLAYBACK_FALLBACKS {
        let playable = match player::resolve(&http, *client, video) {
            Ok(playable) => playable,
            Err(err) => {
                println!("== {client:?} refused: {err}");
                continue;
            }
        };
        let Some(pick) = playable.best_audio().cloned() else {
            println!("== {client:?}: no decodable pick");
            continue;
        };
        println!(
            "== {client:?}: itag {} len {:?} init_end {:?} index_end {:?}",
            pick.tag, pick.content_length, pick.init_end, pick.index_end
        );
        if pick.url.contains("range=") {
            println!("   note: the URL itself carries a `range` parameter");
        }
        let Some(len) = pick.content_length else {
            continue;
        };

        // The rule: it is the requested *end* that decides, not the start.
        println!(
            "   bytes=0-1999999      -> {}",
            ask(&api, &pick.url, 0, 1_999_999)
        );
        println!(
            "   bytes=0-524287       -> {}",
            ask(&api, &pick.url, 0, 524_287)
        );
        println!(
            "   bytes=524288-1048575 -> {}",
            ask(&api, &pick.url, 524_288, 1_048_575)
        );
        println!(
            "   bytes=1048576-...    -> {}",
            ask(&api, &pick.url, 1_048_576, 1_572_863)
        );
        println!(
            "   start inside, end out -> {}",
            ask(&api, &pick.url, 500_000, 2_000_000)
        );
        println!(
            "   start inside, end in  -> {}",
            ask(&api, &pick.url, 500_000, 1_069_000)
        );
        println!(
            "   bytes=0- (no end)     -> {}",
            ask_open_ended(&api, &pick.url)
        );

        // Where the opening ends, to the byte.
        let mut lo = 0u64;
        let mut hi = len.min(8_000_000);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if ask(&api, &pick.url, mid, mid) == "206 Partial Content" {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        println!("   last readable byte {lo}, first refusal {hi}");

        // A fresh signature: same opening, or a new one?
        for attempt in 1..=2 {
            match player::resolve(&http, *client, video)
                .ok()
                .and_then(|fresh| fresh.best_audio().cloned())
            {
                Some(fresh) => println!(
                    "   fresh resolve {attempt}: deep range -> {}",
                    ask(&api, &fresh.url, len / 2, len / 2 + 1023)
                ),
                None => println!("   fresh resolve {attempt}: no pick"),
            }
        }

        // Is there a manifest route that hands out per-segment URLs?
        report_manifests(&http, &api, *client, video);
    }
}

fn ask_open_ended(api: &Api, url: &str) -> String {
    match api.get_stream_range(url, "bytes=0-") {
        Ok(response) => format!("{}", response.status()),
        Err(err) => format!("{err}"),
    }
}

/// Which manifest fields the response offers, and whether fetching one works.
fn report_manifests(http: &InnerTube, api: &Api, client: ClientKind, video: &str) {
    let request = serde_json::json!({
        "videoId": video,
        "contentCheckOk": true,
        "racyCheckOk": true,
    });
    let Ok(raw) = http.post(client, "player", request) else {
        return;
    };
    let data = raw.get("streamingData").cloned().unwrap_or_default();
    let keys: Vec<&str> = data
        .as_object()
        .map(|map| map.keys().map(String::as_str).collect())
        .unwrap_or_default();
    println!("   streamingData keys: {keys:?}");
    for field in ["hlsManifestUrl", "dashManifestUrl"] {
        if let Some(url) = data.get(field).and_then(serde_json::Value::as_str) {
            match api.get_text(url, None) {
                Ok(body) => println!("   {field}: fetched {} chars", body.len()),
                Err(err) => println!("   {field}: fetch failed: {err}"),
            }
        }
    }
}

/// Every client identity worth trying, and whether its URL is capped.
fn clients(video: &str) {
    let http = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("client");
    let api = Api::new(CookieJar::default());

    for (name, context, user_agent) in identities() {
        let body = serde_json::json!({
            "videoId": video,
            "contentCheckOk": true,
            "racyCheckOk": true,
            "context": { "client": context },
        });
        let url = format!("https://www.youtube.com/youtubei/v1/player?key={KEY}&prettyPrint=false");
        let value: serde_json::Value = match http
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("User-Agent", user_agent)
            .json(&body)
            .send()
            .and_then(|response| response.json())
        {
            Ok(value) => value,
            Err(err) => {
                println!("{name:20} transport: {err}");
                continue;
            }
        };
        let status = value
            .pointer("/playabilityStatus/status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("(none)");
        if status != "OK" {
            let reason = value
                .pointer("/playabilityStatus/reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let reason: String = reason.chars().take(60).collect();
            println!("{name:20} {status}: {reason}");
            continue;
        }
        let audio = value
            .pointer("/streamingData/adaptiveFormats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| {
                formats.iter().find(|format| {
                    format
                        .get("mimeType")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|mime| mime.starts_with("audio/mp4"))
                })
            });
        let (Some(audio), Some(url)) = (
            audio,
            audio.and_then(|audio| audio.get("url").and_then(serde_json::Value::as_str)),
        ) else {
            println!("{name:20} OK but no audio/mp4 URL (ciphered)");
            continue;
        };
        let len = audio
            .get("contentLength")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let mid = len.parse::<u64>().unwrap_or(0) / 2;
        let deep = if mid > 0 {
            ask(&api, url, mid, mid + 1023)
        } else {
            "n/a".into()
        };
        println!(
            "{name:20} OK  clen={len:>10}  open={}  deep={deep}",
            ask(&api, url, 0, 1023)
        );
    }
}

/// The identities measured on 2026-10-12. The extra context fields are not
/// decoration: several clients answer a bare name/version pair with a 400.
fn identities() -> Vec<(&'static str, serde_json::Value, &'static str)> {
    vec![
        (
            "ANDROID",
            serde_json::json!({
                "clientName": "ANDROID",
                "clientVersion": "19.35.36",
                "androidSdkVersion": 30,
                "osName": "Android",
                "osVersion": "11",
                "hl": "en",
            }),
            "com.google.android.youtube/19.35.36 (Linux; U; Android 11) gzip",
        ),
        (
            "ANDROID_VR",
            serde_json::json!({
                "clientName": "ANDROID_VR",
                "clientVersion": "1.60.19",
                "deviceMake": "Oculus",
                "deviceModel": "Quest 3",
                "androidSdkVersion": 32,
                "osName": "Android",
                "osVersion": "12",
                "hl": "en",
            }),
            "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12; GB) gzip",
        ),
        (
            "IOS",
            serde_json::json!({
                "clientName": "IOS",
                "clientVersion": "20.10.4",
                "deviceMake": "Apple",
                "deviceModel": "iPhone16,2",
                "osName": "iPhone",
                "osVersion": "18.3.2.22D82",
                "hl": "en",
            }),
            "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)",
        ),
        (
            "IOS_MUSIC",
            serde_json::json!({
                "clientName": "IOS_MUSIC",
                "clientVersion": "7.27.0",
                "deviceMake": "Apple",
                "deviceModel": "iPhone16,2",
                "osName": "iPhone",
                "osVersion": "18.3.2.22D82",
                "hl": "en",
            }),
            "com.google.ios.youtubemusic/7.27.0 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)",
        ),
        (
            "MWEB",
            serde_json::json!({
                "clientName": "MWEB",
                "clientVersion": "2.20240726.01.00",
                "hl": "en",
            }),
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_3_2 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
        ),
        (
            "TVHTML5",
            serde_json::json!({
                "clientName": "TVHTML5",
                "clientVersion": "7.20240724.13.00",
                "hl": "en",
            }),
            "Mozilla/5.0 (PlayStation; PlayStation 4/12.00) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/17.1 Safari/605.1.15",
        ),
        (
            "WEB_EMBEDDED_PLAYER",
            serde_json::json!({
                "clientName": "WEB_EMBEDDED_PLAYER",
                "clientVersion": "1.20240723.01.00",
                "hl": "en",
            }),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        ),
    ]
}

//! The InnerTube transport: endpoints, and which client to ask.
//!
//! InnerTube is the API the official apps call. It is not documented and not
//! contractual, so this module keeps the moving parts — the API key, the client
//! identities, the user agents — in one place with the evidence for each choice.
//!
//! ## Why two clients
//!
//! Measured on 2026-10-10, against the real service:
//!
//! | client | search | playback |
//! |---|---|---|
//! | `WEB` | works, 18 results a page | `playabilityStatus: UNPLAYABLE` — playback now needs a PoToken |
//! | `ANDROID_VR` | returns `sectionListRenderer` with no videos | `OK`, direct URLs — but **only for some videos** |
//! | `IOS` | — | `OK`, direct URLs, and it answers where `ANDROID_VR` refuses |
//! | `TVHTML5_SIMPLY_EMBEDDED_PLAYER` | — | "YouTube is no longer supported in this application or device" |
//!
//! So searching asks `WEB` and resolving a stream asks `ANDROID_VR`. Both are
//! allowed to stop working at any release; [`ClientKind::PLAYBACK_FALLBACKS`] is
//! the list to try, in order, when one does.
//!
//!
//! `ANDROID_VR` is gated per video, not globally: `dQw4w9WgXcQ` resolved fine while
//! long Chinese music compilations came back `LOGIN_REQUIRED` — "Sign in to confirm
//! you're not a bot". `IOS` returned `OK` with two AAC streams for both of those,
//! and its stream is the same shape (`contentLength`, `initRange`, a 206 ranged GET
//! whose head is `ftyp`+`moov`+`sidx`), so it is the fallback. A `visitorData` was
//! tried and did *not* lift the gate. `IOS` answers 400 to a partial context: the
//! device fields *and* a `userAgent`, both inside `context.client`, are required.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_LANGUAGE, CONTENT_TYPE, USER_AGENT};
use serde_json::{json, Value};

/// The public web key every third-party client has used for years.
///
/// Not a secret and not the user's: it identifies the *client*, and it is a
/// constant here so that a change is one edit rather than a hunt. It can be
/// overridden at runtime (see [`InnerTube::with_api_key`]) because YouTube does
/// rotate it occasionally.
pub const API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";

/// Where the anonymous visitor id comes from.
///
/// Any YouTube page carries one; the home page is the cheapest stable one, and it was
/// measured to work as well as the watch page's (2026-10-12).
const YOUTUBE_HOME: &str = "https://www.youtube.com/";

/// How long a failed attempt is believed before the page is asked again.
///
/// A miss is remembered only this long, to stop a burst of requests from each retrying
/// it. Remembering it forever — which is what the first version did — lets one unlucky
/// page decide the whole session, and the app then quietly falls back to a client that
/// serves only the opening.
const VISITOR_RETRY: Duration = Duration::from_secs(30);

/// The visitor id, and when it was last missed.
#[derive(Default)]
struct VisitorCache {
    value: Option<String>,
    failed_at: Option<Instant>,
}

const WEB_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const VR_UA: &str =
    "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12; GB) gzip";

/// The version `IOS` claims. It is its identity and its user agent, and the two
/// must agree.
const IOS_VERSION: &str = "20.10.4";
const IOS_UA: &str = "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)";

/// The visionOS client, taken from what `yt-dlp` sends as its first choice.
///
/// The user agent is a Safari one — this client is a browser engine wearing a headset,
/// not an app, and the `osName`/`osVersion`/`deviceModel` fields are part of that story.
const VISIONOS_VERSION: &str = "1.02";
const VISIONOS_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 \
                           (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const VISIONOS_OS_VERSION: &str = "26.5.23O471";

/// Which client identity a request claims to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// The web player. Search works here; playback does not.
    Web,
    /// The Quest headset client. Returns direct, un-ciphered audio URLs, but is
    /// gated per video.
    AndroidVr,
    /// The iPhone client. Answers with direct URLs for the videos `ANDROID_VR`
    /// refuses.
    Ios,
    /// The headset client. The only one measured *not* to be limited to the opening of a
    /// large file, and it needs no token and no JavaScript runtime — but it will not
    /// serve playback without a [`InnerTube::visitor_data`], and it refuses "made for
    /// kids" videos, which is what the other two are still for.
    VisionOs,
}

impl ClientKind {
    /// The playback clients to try, in order.
    ///
    /// The list exists so that adding a fallback — or removing a dead one — is a one-line
    /// change in the place that documents why. `VISIONOS` goes first because it is the
    /// one that serves a whole large file (measured 2026-10-12); the other two answer
    /// where it refuses, which is "made for kids" videos and a few gated ones.
    pub const PLAYBACK_FALLBACKS: &'static [ClientKind] =
        &[ClientKind::VisionOs, ClientKind::AndroidVr, ClientKind::Ios];

    /// The `context.client` object this identity sends.
    ///
    /// The extra fields are not decoration: `ANDROID_VR` answers a bare
    /// `clientName`/`clientVersion` pair with a 400.
    fn context(self) -> Value {
        match self {
            ClientKind::Web => json!({
                "clientName": "WEB",
                "clientVersion": "2.20240101.00.00",
            }),
            ClientKind::AndroidVr => json!({
                "clientName": "ANDROID_VR",
                "clientVersion": "1.60.19",
                "deviceMake": "Oculus",
                "deviceModel": "Quest 3",
                "osName": "Android",
                "osVersion": "12",
                "androidSdkVersion": 32,
            }),
            // A partial context gets a 400: the device fields *and* the user agent
            // are required, and the user agent belongs inside the context too.
            ClientKind::Ios => json!({
                "clientName": "IOS",
                "clientVersion": IOS_VERSION,
                "deviceMake": "Apple",
                "deviceModel": "iPhone16,2",
                "osName": "iPhone",
                "osVersion": "18.3.2.22D82",
                "userAgent": IOS_UA,
            }),
            ClientKind::VisionOs => json!({
                "clientName": "VISIONOS",
                "clientVersion": VISIONOS_VERSION,
                "deviceMake": "Apple",
                "deviceModel": "RealityDevice17,1",
                "osName": "visionOS",
                "osVersion": VISIONOS_OS_VERSION,
                "userAgent": VISIONOS_UA,
            }),
        }
    }

    fn user_agent(self) -> &'static str {
        match self {
            ClientKind::Web => WEB_UA,
            ClientKind::AndroidVr => VR_UA,
            ClientKind::Ios => IOS_UA,
            ClientKind::VisionOs => VISIONOS_UA,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum YoutubeError {
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("接口返回异常（YouTube 可能要求登录或人机校验）: {0}")]
    NotJson(String),
    #[error("{0}")]
    Api(String),
    #[error("解析失败: {0}")]
    Decode(String),
}

/// A blocking InnerTube client.
///
/// One instance is shared by the API worker, so the connection pool and the
/// cookie-less session are reused; the user agent is set per request because the
/// two identities need different ones.
pub struct InnerTube {
    http: Client,
    api_key: String,
    /// The anonymous id a visit to YouTube leaves behind, fetched at most once.
    ///
    /// Not a login and not a cookie. It is what the `VISIONOS` client needs to be served
    /// playback at all: without it YouTube answers `LOGIN_REQUIRED`, and with it the same
    /// video is served in full (measured 2026-10-12, on five videos from 3 MB to 591 MB).
    ///
    /// The outer `Option` is "has the page been asked", so a failure is not retried on
    /// every request that follows it.
    visitor_data: Mutex<VisitorCache>,
}

impl InnerTube {
    pub fn new() -> Self {
        Self::with_api_key(API_KEY)
    }

    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        // The tuned client mirrors the Bilibili one: a long timeout because a
        // search can be slow, and a short connect timeout so a blocked network
        // fails fast instead of hanging the worker.
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|err| {
                eprintln!("building the YouTube HTTP client failed ({err}); using defaults");
                Client::new()
            });
        Self {
            http,
            api_key: api_key.into(),
            visitor_data: Mutex::new(VisitorCache::default()),
        }
    }

    /// The `visitorData` every request carries, asked for on first use.
    ///
    /// Video-independent session state, so one value serves every video until the process
    /// ends. A miss is not fatal — the request goes out without it and the headset client
    /// answers `LOGIN_REQUIRED`, which the client list then falls through — but it is
    /// retried, and said out loud, because silently falling back is how a client that
    /// serves the opening only gets used without anyone noticing.
    pub fn visitor_data(&self) -> Option<String> {
        {
            let cache = self.visitor_data.lock().unwrap();
            if let Some(value) = &cache.value {
                return Some(value.clone());
            }
            if cache
                .failed_at
                .is_some_and(|at| at.elapsed() < VISITOR_RETRY)
            {
                return None;
            }
        }

        let found = self
            .get_text(ClientKind::Web, YOUTUBE_HOME)
            .ok()
            .and_then(|page| visitor_data_from_page(&page));

        let mut cache = self.visitor_data.lock().unwrap();
        match found {
            Some(value) => {
                cache.value = Some(value.clone());
                cache.failed_at = None;
                Some(value)
            }
            None => {
                cache.failed_at = Some(Instant::now());
                eprintln!(
                    "youtube: {YOUTUBE_HOME} carried no visitor id; playback will fall back \
                     to a client that may serve only the opening"
                );
                None
            }
        }
    }

    /// POST one InnerTube method.
    ///
    /// `body` is the method's own payload; the client context is added here so
    /// every caller cannot forget it.
    pub fn post(
        &self,
        client: ClientKind,
        method: &str,
        mut body: Value,
    ) -> Result<Value, YoutubeError> {
        let url = format!(
            "https://www.youtube.com/youtubei/v1/{method}?key={}&prettyPrint=false",
            self.api_key
        );
        if let Some(object) = body.as_object_mut() {
            let mut context = client.context();
            // The visitor id goes in every client's context. It is what lifts
            // `LOGIN_REQUIRED` on the headset client, and it can only make the others
            // look less like a scraper.
            if let Some(visitor) = self.visitor_data() {
                if let Some(fields) = context.as_object_mut() {
                    fields.insert("visitorData".to_owned(), Value::String(visitor));
                }
            }
            object.insert("context".to_owned(), json!({ "client": context }));
        }

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(client.user_agent()).expect("a constant user agent"),
        );

        let response = self
            .http
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .map_err(|err| YoutubeError::Network(err.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|err| YoutubeError::Network(err.to_string()))?;

        // A blocked or throttled request answers with an HTML page under a 200,
        // so the body has to be checked rather than the status alone.
        serde_json::from_str::<Value>(&text).map_err(|_| {
            YoutubeError::NotJson(format!(
                "HTTP {status}，返回的不是 JSON（{} 字节）",
                text.len()
            ))
        })
    }
}

impl InnerTube {
    /// GET a URL as one of the clients.
    ///
    /// The user agent is not decoration: a `googlevideo`-style URL minted for a
    /// client is bound to it, and asking with a browser agent returns an empty body
    /// rather than an error. Measured on the caption endpoint: six tracks listed,
    /// zero lines parsed, until the request carried the client's own agent.
    pub fn get_text(&self, client: ClientKind, url: &str) -> Result<String, YoutubeError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(client.user_agent()).expect("a constant user agent"),
        );
        self.http
            .get(url)
            .headers(headers)
            .send()
            .map_err(|err| YoutubeError::Network(err.to_string()))?
            .text()
            .map_err(|err| YoutubeError::Network(err.to_string()))
    }
}

/// The anonymous visitor id a YouTube page carries, if it carries one.
///
/// A substring scan rather than a JSON parse: the value lives inside a `<script>` blob
/// that is not the document, and `yt-dlp` reads it the same way.
fn visitor_data_from_page(html: &str) -> Option<String> {
    const NEEDLE: &str = "\"VISITOR_DATA\":\"";
    let start = html.find(NEEDLE)? + NEEDLE.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    (!value.is_empty()).then(|| value.to_owned())
}

impl Default for InnerTube {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The request body must carry the client context, whatever the caller passed.
    #[test]
    fn the_client_context_is_added_to_every_body() {
        assert_eq!(ClientKind::Web.context()["clientName"], "WEB");
        let vr = ClientKind::AndroidVr.context();
        assert_eq!(vr["clientName"], "ANDROID_VR");
        // The fields that make the difference between 200 and 400.
        assert_eq!(vr["androidSdkVersion"], 32);
        assert!(vr["deviceModel"].is_string());
    }

    #[test]
    fn every_identity_has_its_own_user_agent() {
        let mut agents: Vec<&str> = [
            ClientKind::Web,
            ClientKind::AndroidVr,
            ClientKind::Ios,
            ClientKind::VisionOs,
        ]
        .iter()
        .map(|client| client.user_agent())
        .collect();
        let before = agents.len();
        agents.sort_unstable();
        agents.dedup();
        assert_eq!(agents.len(), before, "agents must differ");
    }

    /// The gate is per video, so the list has to have somewhere to fall through to.
    #[test]
    fn playback_has_a_fallback_for_gated_videos() {
        assert_eq!(
            ClientKind::PLAYBACK_FALLBACKS.first(),
            Some(&ClientKind::VisionOs),
            "the headset client is the one that serves a whole large file"
        );
        assert!(
            ClientKind::PLAYBACK_FALLBACKS.contains(&ClientKind::Ios),
            "IOS is what answers when the others say LOGIN_REQUIRED"
        );
        // A partial IOS context is a 400, so the fields that lift it must be there.
        let ios = ClientKind::Ios.context();
        for field in [
            "deviceMake",
            "deviceModel",
            "osName",
            "osVersion",
            "userAgent",
        ] {
            assert!(ios.get(field).is_some(), "IOS needs {field}: {ios}");
        }
        // The same applies to the headset client, whose fields are what make it a
        // headset rather than a bare client name.
        let vision = ClientKind::VisionOs.context();
        assert_eq!(vision["clientName"], "VISIONOS");
        for field in [
            "deviceMake",
            "deviceModel",
            "osName",
            "osVersion",
            "userAgent",
        ] {
            assert!(
                vision.get(field).is_some(),
                "VISIONOS needs {field}: {vision}"
            );
        }
    }

    /// The visitor id is read off a page, and a page without one is not a panic.
    #[test]
    fn the_visitor_id_is_read_off_the_page() {
        let page = r#"<script>var x = {"VISITOR_DATA":"Cgtabc-123","OTHER":1};</script>"#;
        assert_eq!(visitor_data_from_page(page).as_deref(), Some("Cgtabc-123"));

        assert_eq!(visitor_data_from_page("no field here"), None);
        assert_eq!(
            visitor_data_from_page(r#""VISITOR_DATA":"""#),
            None,
            "an empty id is not an id"
        );
        assert_eq!(
            visitor_data_from_page(r#""VISITOR_DATA":"unterminated"#),
            None
        );
    }
}

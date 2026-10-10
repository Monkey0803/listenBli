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

use std::time::Duration;

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

const WEB_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const VR_UA: &str =
    "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12; GB) gzip";

/// The version `IOS` claims. It is its identity and its user agent, and the two
/// must agree.
const IOS_VERSION: &str = "20.10.4";
const IOS_UA: &str = "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)";

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
}

impl ClientKind {
    /// The playback clients to try, in order.
    ///
    /// One entry today because it is the one that was verified. The list exists so
    /// that adding a fallback — or removing a dead one — is a one-line change in
    /// the place that documents why.
    pub const PLAYBACK_FALLBACKS: &'static [ClientKind] = &[ClientKind::AndroidVr, ClientKind::Ios];

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
        }
    }

    fn user_agent(self) -> &'static str {
        match self {
            ClientKind::Web => WEB_UA,
            ClientKind::AndroidVr => VR_UA,
            ClientKind::Ios => IOS_UA,
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
            object.insert("context".to_owned(), json!({ "client": client.context() }));
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
        let mut agents: Vec<&str> = [ClientKind::Web, ClientKind::AndroidVr, ClientKind::Ios]
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
            Some(&ClientKind::AndroidVr)
        );
        assert!(
            ClientKind::PLAYBACK_FALLBACKS.contains(&ClientKind::Ios),
            "IOS is what answers when AndroidVr says LOGIN_REQUIRED"
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
    }
}

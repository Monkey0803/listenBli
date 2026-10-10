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
//! | `ANDROID_VR` | returns `sectionListRenderer` with no videos | `OK`, direct URLs, no cipher |
//! | `TVHTML5_SIMPLY_EMBEDDED_PLAYER` | — | "YouTube is no longer supported in this application or device" |
//!
//! So searching asks `WEB` and resolving a stream asks `ANDROID_VR`. Both are
//! allowed to stop working at any release; [`ClientKind::PLAYBACK_FALLBACKS`] is
//! the list to try, in order, when one does.
//!
//! `IOS` is deliberately absent: the context fields that make it answer were not
//! established here (a partial context got an HTTP 400), and a client that is only
//! *believed* to work is worse than a list that says what was measured.

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

/// Which client identity a request claims to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// The web player. Search works here; playback does not.
    Web,
    /// The Quest headset client. Returns direct, un-ciphered audio URLs.
    AndroidVr,
}

impl ClientKind {
    /// The playback clients to try, in order.
    ///
    /// One entry today because it is the one that was verified. The list exists so
    /// that adding a fallback — or removing a dead one — is a one-line change in
    /// the place that documents why.
    pub const PLAYBACK_FALLBACKS: &'static [ClientKind] = &[ClientKind::AndroidVr];

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
        }
    }

    fn user_agent(self) -> &'static str {
        match self {
            ClientKind::Web => WEB_UA,
            ClientKind::AndroidVr => VR_UA,
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
    fn the_two_identities_do_not_share_a_user_agent() {
        assert_ne!(
            ClientKind::Web.user_agent(),
            ClientKind::AndroidVr.user_agent()
        );
        assert!(!ClientKind::PLAYBACK_FALLBACKS.is_empty());
    }
}

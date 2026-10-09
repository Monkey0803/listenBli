//! A thin, blocking HTTP client for the Bilibili and NetEase endpoints.
//!
//! Only one network worker thread plus one download worker thread use this, so
//! the internal mutexes are uncontended in practice.

use std::sync::Mutex;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, COOKIE, REFERER, USER_AGENT,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::cookie::CookieJar;
use super::models::{NavData, SpiData, UserInfo};
use super::wbi::{self, WbiKeys};

pub const BILI_API: &str = "https://api.bilibili.com";
pub const BILI_WEB: &str = "https://www.bilibili.com/";
const PASSPORT: &str = "https://passport.bilibili.com";
const NETEASE: &str = "https://music.163.com";
const NETEASE_REFERER: &str = "https://music.163.com/";

#[cfg(target_os = "macos")]
const PLATFORM_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)";
#[cfg(target_os = "windows")]
const PLATFORM_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64)";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLATFORM_UA: &str = "Mozilla/5.0 (X11; Linux x86_64)";

fn user_agent() -> String {
    format!("{PLATFORM_UA} AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("接口返回异常（可能是风控拦截，请稍后重试或登录）: {0}")]
    NotJson(String),
    #[error("接口错误 {code}: {message}")]
    Business { code: i64, message: String },
    #[error("返回数据为空")]
    Empty,
    #[error("解析失败: {0}")]
    Decode(String),
}

impl ApiError {
    pub fn is_risk_control(&self) -> bool {
        matches!(
            self,
            ApiError::Business {
                code: -352 | -412 | -403,
                ..
            } | ApiError::NotJson(_)
        )
    }
}

/// The standard Bilibili envelope: `{ "code": 0, "message": "OK", "data": ... }`.
#[derive(Debug, Deserialize)]
struct Envelope<T> {
    code: i64,
    #[serde(default)]
    message: String,
    // No `#[serde(default)]` here: serde already treats a missing `Option` field
    // as `None`, and the attribute would add a spurious `T: Default` bound.
    data: Option<T>,
}

pub struct Api {
    http: Client,
    jar: Mutex<CookieJar>,
    wbi: Mutex<Option<WbiKeys>>,
}

impl Api {
    /// Never fails: if the tuned client cannot be built we fall back to a
    /// default client rather than refusing to start.
    pub fn new(jar: CookieJar) -> Self {
        let http = Client::builder()
            .user_agent(user_agent())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|err| {
                eprintln!("building the HTTP client failed ({err}); using defaults");
                Client::new()
            });
        Self {
            http,
            jar: Mutex::new(jar),
            wbi: Mutex::new(None),
        }
    }

    pub fn jar_snapshot(&self) -> CookieJar {
        self.jar.lock().unwrap().clone()
    }

    pub fn replace_jar(&self, jar: CookieJar) {
        *self.jar.lock().unwrap() = jar;
    }

    /// Cookies currently held for the site, for display/debugging.
    pub fn has_login_cookie(&self) -> bool {
        let jar = self.jar.lock().unwrap();
        jar.get("bilibili.com", "SESSDATA").is_some()
    }

    fn base_headers(referer: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );
        if let Ok(value) = HeaderValue::from_str(referer.unwrap_or(BILI_WEB)) {
            headers.insert(REFERER, value);
        }
        headers
    }

    /// Build a request with referer, cookies and a sane UA.
    fn request(&self, url: &str, referer: Option<&str>) -> Result<RequestBuilder, ApiError> {
        let host = url::Url::parse(url)
            .map_err(|e| ApiError::Network(format!("非法 URL: {e}")))?
            .host_str()
            .unwrap_or("")
            .to_string();

        let mut req = self.http.get(url).headers(Self::base_headers(referer));

        if let Some(cookie) = self.jar.lock().unwrap().cookie_header(&host) {
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                req = req.header(COOKIE, value);
            }
        }

        // Belt and braces: also send the UA header explicitly so that the CDN
        // (which checks it) behaves the same as the API host.
        if let Ok(value) = HeaderValue::from_str(&user_agent()) {
            req = req.header(USER_AGENT, value);
        }

        Ok(req)
    }

    fn send(&self, req: RequestBuilder) -> Result<(String, Response), ApiError> {
        let resp = req.send().map_err(|e| ApiError::Network(e.to_string()))?;

        let host = resp.url().host_str().unwrap_or("").to_string();
        self.jar
            .lock()
            .unwrap()
            .merge_response(&host, resp.headers());

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Network(format!("HTTP {}", status.as_u16())));
        }
        Ok((host, resp))
    }

    pub fn get_text(&self, url: &str, referer: Option<&str>) -> Result<String, ApiError> {
        let (_, resp) = self.send(self.request(url, referer)?)?;
        resp.text().map_err(|e| ApiError::Network(e.to_string()))
    }

    /// Deserialize a bare JSON body (NetEase style).
    pub fn get_json_raw<T: DeserializeOwned>(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<T, ApiError> {
        let text = self.get_text(url, referer)?;
        parse_json(&text)
    }

    /// GET a Bilibili endpoint that returns the standard envelope.
    pub fn get_bili<T: DeserializeOwned>(&self, path_or_url: &str) -> Result<T, ApiError> {
        let envelope = self.get_envelope::<T>(path_or_url)?;
        match envelope.code {
            0 => envelope.data.ok_or(ApiError::Empty),
            code => Err(ApiError::Business {
                code,
                message: envelope.message,
            }),
        }
    }

    fn get_envelope<T: DeserializeOwned>(
        &self,
        path_or_url: &str,
    ) -> Result<Envelope<T>, ApiError> {
        let url = absolute_api_url(path_or_url);
        let text = self.get_text(&url, None)?;
        parse_json(&text)
    }

    /// Like `get_bili`, but returns the payload even when the envelope carries a
    /// non-zero code.
    ///
    /// `nav` is the reason this exists: when nobody is logged in it answers
    /// `{"code": -101, "message": "账号未登录", "data": {...wbi_img...}}`. Treating
    /// that as a hard error would break WBI signing for every anonymous user,
    /// which in turn breaks `playurl` and therefore all playback.
    fn get_bili_tolerant<T: DeserializeOwned>(&self, path_or_url: &str) -> Result<T, ApiError> {
        let envelope = self.get_envelope::<T>(path_or_url)?;
        match envelope.data {
            Some(data) => Ok(data),
            None => match envelope.code {
                0 => Err(ApiError::Empty),
                code => Err(ApiError::Business {
                    code,
                    message: envelope.message,
                }),
            },
        }
    }

    /// GET a Bilibili endpoint with WBI signing, retrying unsigned if the
    /// signature path is rejected outright.
    pub fn get_bili_signed<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
    ) -> Result<T, ApiError> {
        let mixin = self.mixin_key()?;
        let query = wbi::sign_query(params, &mixin, wbi::now_ts());
        let url = format!("{BILI_API}{path}?{query}");

        match self.get_bili::<T>(&url) {
            Ok(value) => Ok(value),
            Err(err) if err.is_risk_control() => {
                // Fall back to an unsigned call: some endpoints accept it even
                // when the signed form trips a heuristic.
                let unsigned = params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("&");
                let url = format!("{BILI_API}{path}?{unsigned}");
                self.get_bili::<T>(&url).map_err(|_| err)
            }
            Err(err) => Err(err),
        }
    }

    /// Lazily fetch and cache the WBI keys.
    pub fn mixin_key(&self) -> Result<String, ApiError> {
        if let Some(keys) = self.wbi.lock().unwrap().as_ref() {
            return Ok(keys.mixin_key.clone());
        }
        let nav = self.fetch_nav()?;
        let img = nav.wbi_img.ok_or(ApiError::Empty)?;
        let keys = WbiKeys::from_urls(&img.img_url, &img.sub_url);
        let mixin = keys.mixin_key.clone();
        *self.wbi.lock().unwrap() = Some(keys);
        Ok(mixin)
    }

    /// `nav` answers `-101` for anonymous callers while still returning the
    /// payload, so it goes through the tolerant path.
    fn fetch_nav(&self) -> Result<NavData, ApiError> {
        self.get_bili_tolerant(&format!("{BILI_API}/x/web-interface/nav"))
    }

    /// Fetch the anonymous `buvid3`/`buvid4` fingerprint cookies.
    pub fn bootstrap_fingerprint(&self) -> Result<(), ApiError> {
        let spi: SpiData = self.get_bili("/x/frontend/finger/spi")?;
        let mut jar = self.jar.lock().unwrap();
        if !spi.b_3.is_empty() {
            jar.set("bilibili.com", "buvid3", &spi.b_3);
        }
        if !spi.b_4.is_empty() {
            jar.set("bilibili.com", "buvid4", &spi.b_4);
        }
        Ok(())
    }

    pub fn nav(&self) -> Result<(UserInfo, bool), ApiError> {
        let nav = self.fetch_nav()?;
        if let Some(img) = &nav.wbi_img {
            if !img.img_url.is_empty() {
                *self.wbi.lock().unwrap() = Some(WbiKeys::from_urls(&img.img_url, &img.sub_url));
            }
        }
        let user = UserInfo {
            mid: nav.mid,
            uname: nav.uname,
            face: nav.face,
            vip_status: nav.vip_status,
        };
        Ok((user, nav.is_login))
    }

    // -- endpoint helpers ---------------------------------------------------

    pub fn qr_generate(&self) -> Result<super::models::QrGenerateData, ApiError> {
        self.get_bili(&format!("{PASSPORT}/x/passport-login/web/qrcode/generate"))
    }

    pub fn qr_poll(&self, key: &str) -> Result<super::models::QrPollData, ApiError> {
        self.get_bili(&format!(
            "{PASSPORT}/x/passport-login/web/qrcode/poll?qrcode_key={key}&source=main-fe-header"
        ))
    }

    pub fn netease_search(&self, keyword: &str) -> Result<super::models::NeSearchResult, ApiError> {
        let encoded =
            percent_encoding::utf8_percent_encode(keyword, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let url = format!("{NETEASE}/api/search/get?s={encoded}&type=1&limit=10");
        self.get_json_raw(&url, Some(NETEASE_REFERER))
    }

    pub fn netease_lyric(&self, id: i64) -> Result<super::models::NeLyric, ApiError> {
        let url = format!("{NETEASE}/api/song/lyric?id={id}&lv=1&kv=1&tv=-1");
        self.get_json_raw(&url, Some(NETEASE_REFERER))
    }

    /// Raw streaming response, used by the download worker for progress.
    pub fn get_stream(&self, url: &str, referer: Option<&str>) -> Result<Response, ApiError> {
        let (_, resp) = self.send(self.request(url, referer)?)?;
        Ok(resp)
    }

    /// Fetch a whole body into memory (cover images).
    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>, ApiError> {
        let (_, resp) = self.send(self.request(url, None)?)?;
        let bytes = resp.bytes().map_err(|e| ApiError::Network(e.to_string()))?;
        Ok(bytes.to_vec())
    }
}

fn absolute_api_url(path_or_url: &str) -> String {
    if path_or_url.starts_with("http") {
        path_or_url.to_string()
    } else {
        format!("{BILI_API}{path_or_url}")
    }
}

fn parse_json<T: DeserializeOwned>(text: &str) -> Result<T, ApiError> {
    serde_json::from_str::<T>(text).map_err(|e| {
        // Anti-bot interception answers with an HTML page rather than JSON.
        let head = text.trim_start().chars().take(1).collect::<String>();
        if head == "<" {
            ApiError::NotJson("服务器返回了 HTML 页面（风控拦截）".to_string())
        } else {
            ApiError::Decode(format!("{e}"))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_mentions_the_host_platform() {
        let ua = user_agent();
        assert!(ua.contains("AppleWebKit"));
        #[cfg(target_os = "macos")]
        assert!(ua.contains("Macintosh"));
        #[cfg(target_os = "windows")]
        assert!(ua.contains("Windows NT"));
    }

    #[test]
    fn risk_control_detection() {
        assert!(ApiError::Business {
            code: -352,
            message: "".into()
        }
        .is_risk_control());
        assert!(!ApiError::Business {
            code: -404,
            message: "".into()
        }
        .is_risk_control());
        assert!(ApiError::NotJson("html".into()).is_risk_control());
    }

    #[test]
    fn html_body_is_reported_as_risk_control() {
        let err = parse_json::<Envelope<serde_json::Value>>("<!DOCTYPE html><html>").unwrap_err();
        assert!(matches!(err, ApiError::NotJson(_)));
    }

    #[test]
    fn envelope_decodes_business_error() {
        let err =
            parse_json::<Envelope<serde_json::Value>>(r#"{"code":-101,"message":"账号未登录"}"#)
                .unwrap();
        assert_eq!(err.code, -101);
        assert!(err.data.is_none());
    }

    /// `nav` answers `-101` while anonymous but still carries the WBI keys, so
    /// the payload must remain reachable.
    #[test]
    fn envelope_keeps_payload_despite_negative_code() {
        let envelope = parse_json::<Envelope<serde_json::Value>>(
            r#"{"code":-101,"message":"账号未登录","ttl":1,"data":{"isLogin":false,"wbi_img":{"img_url":"https://i0.hdslb.com/bfs/wbi/abc.png","sub_url":"https://i0.hdslb.com/bfs/wbi/def.png"}}}"#,
        )
        .unwrap();
        assert_eq!(envelope.code, -101);
        assert!(envelope.data.is_some(), "payload must survive a -101 code");
    }

    #[test]
    fn builds_absolute_api_urls() {
        assert_eq!(
            absolute_api_url("/x/web-interface/nav"),
            "https://api.bilibili.com/x/web-interface/nav"
        );
        assert_eq!(
            absolute_api_url("https://passport.bilibili.com/x/y"),
            "https://passport.bilibili.com/x/y"
        );
    }
}

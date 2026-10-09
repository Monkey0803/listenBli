//! QR-code login.
//!
//! `poll` returns HTTP 200 with `code: 0` at the envelope level; the real status
//! lives in `data.code`. On success the response also carries `Set-Cookie`
//! headers which `Api` has already merged into its jar by the time we look.

use super::client::{Api, ApiError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrStatus {
    /// 86101 — not scanned yet.
    Pending,
    /// 86090 — scanned, waiting for the user to confirm on their phone.
    Scanned,
    /// 0 — confirmed and cookies have been issued.
    Confirmed,
    /// 86038 — the QR code timed out and must be regenerated.
    Expired,
    Unknown(i64, String),
}

pub fn generate(api: &Api) -> Result<(String, String), ApiError> {
    let data = api.qr_generate()?;
    if data.qrcode_key.is_empty() {
        return Err(ApiError::Empty);
    }
    Ok((data.qrcode_key, data.url))
}

/// Returns the status plus, on success, the cross-domain URL that repeats the
/// credentials as query parameters (used as a fallback cookie source).
pub fn poll(api: &Api, key: &str) -> Result<(QrStatus, Option<String>), ApiError> {
    let data = match api.qr_poll(key) {
        Ok(data) => data,
        // An expired key can come back with `data: null`.
        Err(ApiError::Empty) => return Ok((QrStatus::Expired, None)),
        Err(err) => return Err(err),
    };

    let status = match data.code {
        0 => QrStatus::Confirmed,
        86101 => QrStatus::Pending,
        86090 => QrStatus::Scanned,
        86038 => QrStatus::Expired,
        other => QrStatus::Unknown(other, data.message),
    };

    let url = if data.url.is_empty() {
        None
    } else {
        Some(data.url)
    };
    Ok((status, url))
}

/// Parse the credentials out of the cross-domain URL's query string.
///
/// This is the fallback path for when `Set-Cookie` did not yield `SESSDATA`.
pub fn cookies_from_cross_domain_url(url: &str) -> Vec<(String, String)> {
    let Ok(parsed) = url::Url::parse(url) else {
        return Vec::new();
    };
    const WANTED: [&str; 4] = ["SESSDATA", "bili_jct", "DedeUserID", "DedeUserID__ckMd5"];
    parsed
        .query_pairs()
        .filter(|(k, _)| WANTED.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_credentials_from_cross_domain_url() {
        let url = "https://passport.bilibili.com/login?SESSDATA=abc%2Cdef&bili_jct=xyz&DedeUserID=42&DedeUserID__ckMd5=md5&timestamp=1&gourl=https%3A%2F%2Fwww.bilibili.com";
        let cookies = cookies_from_cross_domain_url(url);
        assert_eq!(cookies.len(), 4);
        assert_eq!(cookies[0].0, "SESSDATA");
        assert_eq!(cookies[0].1, "abc,def");
        assert!(cookies.iter().any(|(k, v)| k == "DedeUserID" && v == "42"));
    }

    #[test]
    fn ignores_unrelated_url() {
        assert!(cookies_from_cross_domain_url("not a url").is_empty());
        assert!(cookies_from_cross_domain_url("https://example.com/?a=b").is_empty());
    }
}

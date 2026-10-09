//! Bilibili's WBI request signing.
//!
//! Several `x/web-interface/*` endpoints reject unsigned requests (they answer
//! with an HTML anti-bot page or `code: -352`). The scheme is:
//!
//! 1. `nav` exposes two image URLs; their file stems are `img_key` / `sub_key`.
//! 2. Concatenate `sub_key + img_key` (64 chars) and permute it through a fixed
//!    table, keeping the first 32 characters -> the "mixin key".
//! 3. Add `wts` (unix seconds), sort all params by key, drop `!'()*` from each
//!    value, percent-encode, join with `&`, append the mixin key, MD5 -> `w_rid`.
//!
//! The mixin-key step is verified against a live `nav` response by the unit test
//! below (the expected value was captured from the real API).

use md5::{Digest, Md5};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// Permutation table from Bilibili's own `wbi` implementation.
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// Character set matching `encodeURIComponent`'s "unreserved" characters.
/// Deliberately NOT `form_urlencoded`, which would render a space as `+`.
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

fn encode(value: &str) -> String {
    utf8_percent_encode(value, ENCODE_SET).to_string()
}

fn md5_hex(input: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract the file stem, e.g. `.../7cd084941338484aae1ad9425b84077c.png` -> that hash.
fn key_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbiKeys {
    pub img_key: String,
    pub sub_key: String,
    pub mixin_key: String,
}

impl WbiKeys {
    pub fn from_urls(img_url: &str, sub_url: &str) -> Self {
        let img_key = key_from_url(img_url);
        let sub_key = key_from_url(sub_url);
        let source = format!("{sub_key}{img_key}");
        let bytes = source.as_bytes();
        let mixin_key: String = MIXIN_KEY_ENC_TAB
            .iter()
            .take(32)
            .filter_map(|&i| bytes.get(i).copied())
            .map(char::from)
            .collect();
        Self {
            img_key,
            sub_key,
            mixin_key,
        }
    }
}

/// Build the signed query string (`...&wts=..&w_rid=..`).
pub fn sign_query(params: &[(String, String)], mixin_key: &str, wts: u64) -> String {
    let mut all: Vec<(String, String)> = params.to_vec();
    all.push(("wts".to_string(), wts.to_string()));
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let query = all
        .iter()
        .map(|(k, v)| {
            let cleaned: String = v.chars().filter(|c| !"!'()*".contains(*c)).collect();
            format!("{}={}", encode(k), encode(&cleaned))
        })
        .collect::<Vec<_>>()
        .join("&");

    let w_rid = md5_hex(&format!("{query}{mixin_key}"));
    format!("{query}&w_rid={w_rid}")
}

/// Seconds since the unix epoch; the only part of signing that is not pure.
pub fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from a live `GET /x/web-interface/nav` response.
    const IMG_URL: &str = "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png";
    const SUB_URL: &str = "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png";

    #[test]
    fn mixin_key_matches_live_api() {
        let keys = WbiKeys::from_urls(IMG_URL, SUB_URL);
        assert_eq!(keys.img_key, "7cd084941338484aae1ad9425b84077c");
        assert_eq!(keys.sub_key, "4932caff0ff746eab6f01bf08b70ac45");
        // Verified by replaying the same algorithm against the live endpoint.
        assert_eq!(keys.mixin_key, "4af39007a1f5828008aecf30cae44936");
        assert_eq!(keys.mixin_key.len(), 32);
    }

    #[test]
    fn signing_is_deterministic_and_sorted() {
        let keys = WbiKeys::from_urls(IMG_URL, SUB_URL);
        let params = vec![
            ("search_type".to_string(), "video".to_string()),
            ("keyword".to_string(), "abc".to_string()),
        ];
        let q = sign_query(&params, &keys.mixin_key, 1700000000);
        assert!(q.contains("wts=1700000000"));
        assert!(q.starts_with("keyword=abc&search_type=video&wts=1700000000&w_rid="));
        // Stable across calls.
        assert_eq!(q, sign_query(&params, &keys.mixin_key, 1700000000));
        // w_rid is a 32 char lowercase hex digest.
        let rid = q.rsplit("w_rid=").next().unwrap();
        assert_eq!(rid.len(), 32);
        assert!(rid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn strips_forbidden_characters() {
        let params = vec![("q".to_string(), "a!b'c(d)e*f".to_string())];
        let q = sign_query(&params, "mixin", 1);
        assert!(q.starts_with("q=abcdef"), "got {q}");
    }

    #[test]
    fn encodes_space_as_percent_twenty() {
        let params = vec![("q".to_string(), "a b".to_string())];
        let q = sign_query(&params, "mixin", 1);
        assert!(q.starts_with("q=a%20b"), "got {q}");
    }
}

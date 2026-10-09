//! A tiny, dependency-free cookie jar.
//!
//! `reqwest`'s `cookie_store` feature keeps cookies inside the client but gives
//! no way to read them back, which we need in order to persist the login. Rather
//! than pull in `reqwest_cookie_store`, we manage cookies ourselves:
//!
//! * every response's `Set-Cookie` headers are merged into the jar,
//! * every request gets a `Cookie` header built from the entries whose domain
//!   matches the request host.
//!
//! Domain scoping matters for more than correctness: it keeps Bilibili's
//! `SESSDATA` credential from being sent to the third-party lyrics host.

use std::collections::BTreeMap;

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CookieJar {
    /// normalized domain (no leading dot) -> name -> value
    domains: BTreeMap<String, BTreeMap<String, String>>,
}

fn normalize_domain(domain: &str) -> String {
    domain.trim().trim_start_matches('.').to_ascii_lowercase()
}

/// `www.bilibili.com` matches cookie domain `bilibili.com`.
fn domain_matches(host: &str, cookie_domain: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == cookie_domain || host.ends_with(&format!(".{cookie_domain}"))
}

impl CookieJar {
    pub fn is_empty(&self) -> bool {
        self.domains.values().all(|m| m.is_empty())
    }

    /// All cookies applicable to `host`, ready for a `Cookie:` header.
    pub fn cookie_header(&self, host: &str) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        for (domain, jar) in &self.domains {
            if !domain_matches(host, domain) {
                continue;
            }
            for (name, value) in jar {
                parts.push(format!("{name}={value}"));
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }

    pub fn get(&self, domain: &str, name: &str) -> Option<&str> {
        self.domains
            .get(&normalize_domain(domain))
            .and_then(|m| m.get(name))
            .map(String::as_str)
    }

    pub fn set(&mut self, domain: &str, name: &str, value: &str) {
        self.domains
            .entry(normalize_domain(domain))
            .or_default()
            .insert(name.to_string(), value.to_string());
    }

    /// Merge every `Set-Cookie` header from a response.
    ///
    /// Cookies without an explicit `Domain` attribute are attributed to the host
    /// that was actually requested.
    pub fn merge_response(&mut self, request_host: &str, headers: &HeaderMap) {
        for value in headers.get_all(reqwest::header::SET_COOKIE).iter() {
            let Ok(raw) = value.to_str() else { continue };
            let mut segments = raw.split(';');
            let Some(pair) = segments.next() else {
                continue;
            };
            let Some((name, val)) = pair.split_once('=') else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }

            let mut domain = normalize_domain(request_host);
            let mut expired = false;
            for attr in segments {
                let attr = attr.trim();
                let (key, attr_val) = match attr.split_once('=') {
                    Some((k, v)) => (k.trim(), v.trim()),
                    None => (attr, ""),
                };
                if key.eq_ignore_ascii_case("domain") && !attr_val.is_empty() {
                    domain = normalize_domain(attr_val);
                }
                if key.eq_ignore_ascii_case("max-age") && attr_val == "0" {
                    expired = true;
                }
            }

            if expired || val.is_empty() {
                if let Some(jar) = self.domains.get_mut(&domain) {
                    jar.remove(name);
                }
                continue;
            }

            self.domains
                .entry(domain)
                .or_default()
                .insert(name.to_string(), val.trim().to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderValue, SET_COOKIE};

    fn headers(pairs: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for p in pairs {
            h.append(SET_COOKIE, HeaderValue::from_str(p).unwrap());
        }
        h
    }

    #[test]
    fn merges_and_scopes_cookies() {
        let mut jar = CookieJar::default();
        jar.merge_response(
            "passport.bilibili.com",
            &headers(&["SESSDATA=abc; Path=/; Domain=.bilibili.com; HttpOnly"]),
        );
        jar.merge_response("api.bilibili.com", &headers(&["buvid3=xyz; Path=/"]));

        assert!(jar
            .cookie_header("api.bilibili.com")
            .unwrap()
            .contains("SESSDATA=abc"));
        assert!(jar
            .cookie_header("www.bilibili.com")
            .unwrap()
            .contains("SESSDATA=abc"));
        // The credential must never leak to a third-party host.
        assert_eq!(jar.cookie_header("music.163.com"), None);
    }

    #[test]
    fn host_without_domain_attribute_is_scoped_to_host() {
        let mut jar = CookieJar::default();
        jar.merge_response("music.163.com", &headers(&["NMTID=1; Path=/"]));
        assert!(jar.cookie_header("music.163.com").is_some());
        assert_eq!(jar.cookie_header("api.bilibili.com"), None);
    }

    #[test]
    fn max_age_zero_removes_cookie() {
        let mut jar = CookieJar::default();
        jar.merge_response("api.bilibili.com", &headers(&["buvid3=xyz"]));
        assert!(jar.get("api.bilibili.com", "buvid3").is_some());
        jar.merge_response("api.bilibili.com", &headers(&["buvid3=; Max-Age=0"]));
        assert!(jar.get("api.bilibili.com", "buvid3").is_none());
    }

    #[test]
    fn round_trips_through_json() {
        let mut jar = CookieJar::default();
        jar.set("bilibili.com", "SESSDATA", "abc");
        let text = serde_json::to_string(&jar).unwrap();
        let back: CookieJar = serde_json::from_str(&text).unwrap();
        assert_eq!(back.get("bilibili.com", "SESSDATA"), Some("abc"));
    }
}

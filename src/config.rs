//! Persisted user settings, including the login cookies.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::api::cookie::CookieJar;
use crate::platform;

/// How many recent search keywords are kept.
pub const MAX_SEARCH_HISTORY: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Prefer the VIP-only lossless FLAC stream when the account can see it.
    #[serde(default)]
    pub prefer_flac: bool,
    /// Show the NetEase translation line under the original when available.
    #[serde(default = "default_true")]
    pub prefer_translation: bool,
    /// Recent search keywords, most recent first.
    #[serde(default)]
    pub search_history: Vec<String>,
    /// Override the auto-detected system CJK font.
    #[serde(default)]
    pub cjk_font_path: Option<PathBuf>,
    /// Which system CJK face to use; `auto` probes the platform.
    #[serde(default = "default_cjk_font")]
    pub cjk_font: String,
    /// Accent preset id: `pink`, `cyan` or `violet`.
    #[serde(default = "default_accent")]
    pub accent: String,
    /// Lyric body size in points.
    #[serde(default = "default_lyric_size")]
    pub lyric_size: f32,
    /// `comfortable` (62px rows) or `compact` (52px rows).
    #[serde(default = "default_density")]
    pub density: String,
    /// Where cached audio and lyrics live, when the user wants them somewhere
    /// other than the platform's cache directory.
    ///
    /// Deliberately independent of where this file itself is stored: moving the
    /// cache must never move the config, which holds the login credentials.
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub cookies: CookieJar,
}

fn default_version() -> u32 {
    1
}

fn default_volume() -> f32 {
    0.8
}

fn default_true() -> bool {
    true
}

fn default_cjk_font() -> String {
    "auto".to_owned()
}

fn default_accent() -> String {
    "pink".to_owned()
}

fn default_lyric_size() -> f32 {
    15.5
}

fn default_density() -> String {
    "comfortable".to_owned()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            volume: default_volume(),
            prefer_flac: false,
            prefer_translation: true,
            search_history: Vec::new(),
            cjk_font_path: None,
            cjk_font: default_cjk_font(),
            accent: default_accent(),
            lyric_size: default_lyric_size(),
            density: default_density(),
            cache_dir: None,
            cookies: CookieJar::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        platform::config_dir().join("config.json")
    }

    /// Never fails: a corrupt or absent file yields defaults so that a bad
    /// config can not stop the application from starting.
    /// Record `keyword` as the most recent search.
    ///
    /// Searching something twice moves it back to the top instead of adding a
    /// duplicate, and the list never grows past [`MAX_SEARCH_HISTORY`].
    pub fn remember_search(&mut self, keyword: &str) {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            return;
        }
        let lower = keyword.to_lowercase();
        self.search_history
            .retain(|older| older.to_lowercase() != lower);
        self.search_history.insert(0, keyword.to_owned());
        self.search_history.truncate(MAX_SEARCH_HISTORY);
    }

    pub fn load() -> Self {
        let path = Self::path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str::<Config>(&text) {
            Ok(config) => config,
            Err(err) => {
                eprintln!(
                    "config at {} is unreadable ({err}); using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        let text = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        platform::write_private(&path, &text)
    }
}

pub type SharedConfig = Arc<Mutex<Config>>;

pub fn shared(config: Config) -> SharedConfig {
    Arc::new(Mutex::new(config))
}

/// Convenience for the common "read one field" case.
pub fn with<T>(config: &SharedConfig, f: impl FnOnce(&Config) -> T) -> T {
    f(&config.lock().unwrap())
}

pub fn config_path_display() -> String {
    Config::path().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_dir_defaults_to_unset_and_round_trips() {
        // Absent in an older config file: must load as "use the default".
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value.as_object_mut().unwrap().remove("cache_dir");
        let loaded: Config = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.cache_dir, None);

        let config = Config {
            cache_dir: Some(PathBuf::from("/Volumes/Big/listenbli-cache")),
            ..Config::default()
        };
        let text = serde_json::to_string(&config).unwrap();
        let back: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(
            back.cache_dir,
            Some(PathBuf::from("/Volumes/Big/listenbli-cache"))
        );
    }

    #[test]
    fn defaults_are_sane() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert!((config.volume - 0.8).abs() < f32::EPSILON);
        assert!(config.prefer_translation);
        assert!(!config.prefer_flac);
        assert!(config.cookies.is_empty());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // Simulates an older config file that predates a field.
        let config: Config = serde_json::from_str(r#"{"version":1}"#).unwrap();
        assert!(config.prefer_translation);
        assert!((config.volume - 0.8).abs() < f32::EPSILON);
        assert_eq!(config.accent, "pink");
        assert_eq!(config.density, "comfortable");
        assert_eq!(config.cjk_font, "auto");
        assert!((config.lyric_size - 15.5).abs() < f32::EPSILON);
        assert!(config.search_history.is_empty());
    }

    #[test]
    fn round_trips_including_cookies() {
        let mut config = Config::default();
        config.cookies.set("bilibili.com", "SESSDATA", "secret");
        config.prefer_flac = true;
        let text = serde_json::to_string(&config).unwrap();
        let back: Config = serde_json::from_str(&text).unwrap();
        assert!(back.prefer_flac);
        assert_eq!(back.cookies.get("bilibili.com", "SESSDATA"), Some("secret"));
    }

    #[test]
    fn search_history_keeps_the_newest_first() {
        let mut config = Config::default();
        config.remember_search("周杰伦 晴天");
        config.remember_search("米津玄師");
        assert_eq!(config.search_history, ["米津玄師", "周杰伦 晴天"]);
    }

    #[test]
    fn searching_again_moves_a_keyword_back_to_the_top() {
        let mut config = Config::default();
        for keyword in ["a", "b", "c"] {
            config.remember_search(keyword);
        }
        // Re-searching an old keyword promotes it instead of duplicating it,
        // and matching ignores case.
        config.remember_search("A");
        assert_eq!(config.search_history, ["A", "c", "b"]);
    }

    #[test]
    fn search_history_is_capped_and_trims_blank_queries() {
        let mut config = Config::default();
        for index in 0..MAX_SEARCH_HISTORY + 4 {
            config.remember_search(&format!("query {index}"));
        }
        assert_eq!(config.search_history.len(), MAX_SEARCH_HISTORY);
        assert_eq!(
            config.search_history[0],
            format!("query {}", MAX_SEARCH_HISTORY + 3)
        );

        config.remember_search("   ");
        assert_eq!(config.search_history.len(), MAX_SEARCH_HISTORY);
        assert!(!config
            .search_history
            .iter()
            .any(|entry| entry.trim().is_empty()));
    }

    #[test]
    fn search_history_round_trips_through_json() {
        let mut config = Config::default();
        config.remember_search("晴天");
        let back: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.search_history, ["晴天"]);
    }

    #[test]
    fn config_path_lives_under_the_platform_config_dir() {
        let path = Config::path();
        assert!(path.starts_with(platform::config_dir()));
        assert!(path.is_absolute());
    }
}

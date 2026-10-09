//! Everything that differs between macOS and Windows lives in this module so the
//! rest of the code base never has to care about the host platform.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

const APP: &str = "listenBli";

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", APP)
}

/// `~/Library/Application Support/listenBli` on macOS,
/// `%APPDATA%\listenBli\config` on Windows.
pub fn config_dir() -> PathBuf {
    match project_dirs() {
        Some(d) => d.config_dir().to_path_buf(),
        None => fallback_dir("config"),
    }
}

/// `~/Library/Caches/listenBli` on macOS,
/// `%LOCALAPPDATA%\listenBli\cache` on Windows.
pub fn cache_dir() -> PathBuf {
    match project_dirs() {
        Some(d) => d.cache_dir().to_path_buf(),
        None => fallback_dir("cache"),
    }
}

fn fallback_dir(kind: &str) -> PathBuf {
    std::env::temp_dir().join(APP).join(kind)
}

/// Create a directory (and its parents) if it does not exist yet.
pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Windows fonts live under `%SystemRoot%\Fonts`; never hardcode `C:\Windows`
/// because Windows may be installed on another drive.
fn windows_fonts_dir() -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("Fonts")
}

/// Ordered candidate list of system fonts with CJK coverage.
pub fn cjk_font_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        for p in [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Supplemental/Songti.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
        ] {
            out.push(PathBuf::from(p));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let dir = windows_fonts_dir();
        for f in [
            "msyh.ttc", // Microsoft YaHei
            "msyh.ttf",
            "msyhl.ttc",
            "simhei.ttf", // SimHei
            "simsun.ttc", // SimSun
            "nsimsun.ttc",
            "Deng.ttf", // DengXian (Windows 10+)
            "DengXian.ttf",
        ] {
            out.push(dir.join(f));
        }
    }

    // Not a supported target, but keeps the crate buildable on a Linux dev box.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        for p in [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
        ] {
            out.push(PathBuf::from(p));
        }
    }

    out
}

/// First existing candidate wins; an explicit user override is checked first.
pub fn find_cjk_font(override_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    cjk_font_candidates().into_iter().find(|p| p.is_file())
}

/// The font choices offered by the settings sheet, in display order. `auto`
/// means "whatever the platform probe finds".
pub const CJK_FONT_CHOICES: [(&str, &str); 6] = [
    ("auto", "自动探测"),
    ("pingfang", "PingFang SC"),
    ("hiragino", "Hiragino Sans GB"),
    ("stheiti", "STHeiti"),
    ("songti", "Songti SC"),
    ("yahei", "微软雅黑"),
];

/// Paths behind one of [`CJK_FONT_CHOICES`]. Empty for `auto` and for choices
/// whose fonts are not installed on this platform.
fn cjk_choice_paths(choice: &str) -> Vec<PathBuf> {
    let macos = |names: &[&str]| -> Vec<PathBuf> {
        if !cfg!(target_os = "macos") {
            return Vec::new();
        }
        names
            .iter()
            .map(|n| PathBuf::from("/System/Library/Fonts/Supplemental").join(n))
            .chain(
                names
                    .iter()
                    .map(|n| PathBuf::from("/System/Library/Fonts").join(n)),
            )
            .collect()
    };
    match choice {
        "pingfang" => macos(&["PingFang.ttc"]),
        "hiragino" => macos(&["Hiragino Sans GB.ttc"]),
        "stheiti" => macos(&["STHeiti Light.ttc", "STHeiti Medium.ttc"]),
        "songti" => macos(&["Songti.ttc"]),
        "yahei" => {
            if cfg!(target_os = "windows") {
                let dir = windows_fonts_dir();
                vec![dir.join("msyh.ttc"), dir.join("msyh.ttf")]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Resolve the CJK face from a settings choice plus an optional explicit path.
///
/// The explicit path always wins; otherwise the preset is probed and, failing
/// that, the platform defaults.
pub fn resolve_cjk_font(choice: &str, override_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    cjk_choice_paths(choice)
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| find_cjk_font(None))
}

/// A CJK serif for the lyric body, so songs read differently from the chrome.
pub fn lyric_font() -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Supplemental/Songti.ttc",
            "/System/Library/Fonts/Supplemental/NotoSerifSC-Regular.otf",
            "/Library/Fonts/Songti.ttc",
        ]
    } else if cfg!(target_os = "windows") {
        &[]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSerifCJK-Regular.ttc",
        ]
    };
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| {
            if cfg!(target_os = "windows") {
                let dir = windows_fonts_dir();
                // SimSun is the closest thing to a serif that ships everywhere.
                ["simsun.ttc", "nsimsun.ttc"]
                    .iter()
                    .map(|f| dir.join(f))
                    .find(|p| p.is_file())
            } else {
                None
            }
        })
}

/// A monospaced face for durations, UIDs and paths (SF Mono on macOS).
pub fn mono_font() -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/SFNSMono.ttf",
            "/System/Library/Fonts/Menlo.ttc",
        ]
    } else if cfg!(target_os = "windows") {
        &[]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        ]
    };
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| {
            if cfg!(target_os = "windows") {
                let dir = windows_fonts_dir();
                ["consola.ttf", "CascadiaMono.ttf"]
                    .iter()
                    .map(|f| dir.join(f))
                    .find(|p| p.is_file())
            } else {
                None
            }
        })
}

/// A placeholder for the font-path field, filled with a path that exists on
/// this machine when possible.
pub fn cjk_font_hint() -> String {
    find_cjk_font(None)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/path/to/font.ttc".to_owned())
}

/// Best-effort tightening of file permissions for credential files.
///
/// Unix gets `0600`. On Windows we do nothing: the file already lives under the
/// per-user `%APPDATA%` profile, whose ACL is only readable by that user and
/// administrators, and `PermissionsExt` does not exist there.
pub fn restrict_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(path)?.permissions();
        perm.set_mode(0o600);
        fs::set_permissions(path, perm)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Write a file atomically-ish and restrict its permissions.
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, contents)?;
    restrict_permissions(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_candidates_are_absolute_paths() {
        let candidates = cjk_font_candidates();
        assert!(
            !candidates.is_empty(),
            "expected at least one font candidate"
        );
        for c in &candidates {
            assert!(c.is_absolute(), "font candidate must be absolute: {c:?}");
        }
    }

    #[test]
    fn override_wins_when_it_exists() {
        let tmp = std::env::temp_dir().join("listenbli-font-override-test.ttf");
        fs::write(&tmp, b"not really a font").unwrap();
        let found = find_cjk_font(Some(&tmp));
        assert_eq!(found.as_deref(), Some(tmp.as_path()));
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn missing_override_falls_back_to_candidates() {
        let missing = PathBuf::from("/definitely/not/here/font.ttf");
        // Should not panic; either finds a real system font or nothing at all.
        let _ = find_cjk_font(Some(&missing));
    }

    #[test]
    fn config_and_cache_dirs_are_absolute() {
        assert!(config_dir().is_absolute());
        assert!(cache_dir().is_absolute());
    }
}

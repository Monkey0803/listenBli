//! Everything that differs between macOS and Windows lives in this module so the
//! rest of the code base never has to care about the host platform.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Total size of everything under `path`, in bytes.
///
/// Best effort: entries that cannot be read are skipped, because this only feeds
/// a "how much is the cache using" line and must never fail or stall a frame.
/// The cache holds a few hundred files, so one walk is sub-millisecond.
pub fn dir_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            total += dir_bytes(&entry.path());
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

/// Show a directory in the OS file manager.
///
/// The directory is created if missing, so the button never fails just because
/// nothing has been cached yet.
pub fn reveal_dir(path: &Path) -> Result<(), String> {
    let mut command = reveal_command(path)?;
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("打开文件夹失败：{err}"))
}

/// Validate the path and build the launcher, without running it.
///
/// Split out so the refusal is testable: a passing case would open a real window
/// during `cargo test`.
fn reveal_command(path: &Path) -> Result<Command, String> {
    let _ = ensure_dir(path);
    if !path.is_dir() {
        return Err(format!("目录不存在：{}", path.display()));
    }

    // `Command` passes the path as one argument and never involves a shell, so a
    // path containing spaces or metacharacters cannot become a second command.
    #[cfg(target_os = "macos")]
    let command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };
    #[cfg(target_os = "windows")]
    let command = {
        let mut command = Command::new("explorer");
        command.arg(path);
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    Ok(command)
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

/// Hand `url` to whatever the system uses for `https` links.
///
/// The URL is checked first: the Windows path goes through `cmd /C start`, and a
/// string built from a network response is the last thing that should reach a
/// shell unexamined. Only `https://` with characters a bilibili URL can contain
/// gets through, so `javascript:` and friends are refused rather than launched.
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_safe_https(url) {
        return Err(format!("拒绝打开可疑链接：{url}"));
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };
    // `start` needs its own title argument, otherwise it treats a quoted URL as
    // the window title.
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("打开浏览器失败：{err}"))
}

/// Whether `url` is an `https` URL made only of characters that can appear in
/// one, which is what makes it safe to pass to a shell.
fn is_safe_https(url: &str) -> bool {
    const ALLOWED: &str = "-._~:/?#[]@!$&'()*+,;=%";
    url.strip_prefix("https://").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ALLOWED.contains(c))
    })
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

/// The app artwork, decoded from the PNG embedded at build time.
///
/// macOS does not need it — the Dock and Finder read
/// `Contents/Resources/ListenBli.icns`, and the window hands eframe an empty
/// icon so it leaves that alone. Windows has no bundle icon to fall back on:
/// without this, eframe installs its own egui placeholder, which is what the
/// taskbar and alt-tab would show. `assets/icon.ico` covers the executable's
/// own icon, which Explorer reads (see `build.rs`).
pub fn app_icon() -> Option<egui::IconData> {
    match eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        Ok(icon) => Some(icon),
        Err(err) => {
            eprintln!("内置应用图标无法解析：{err}");
            None
        }
    }
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

    #[test]
    fn dir_bytes_sums_nested_files_and_ignores_a_missing_root() {
        let root = std::env::temp_dir().join(format!("listenbli-bytes-{}", std::process::id()));
        let nested = root.join("lyrics").join("v2");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("a.bin"), vec![0u8; 1000]).unwrap();
        fs::write(nested.join("b.bin"), vec![0u8; 24]).unwrap();

        assert_eq!(dir_bytes(&root), 1024);
        // A directory that does not exist reads as empty rather than failing:
        // this only ever feeds a label.
        assert_eq!(dir_bytes(&root.join("nope")), 0);

        let _ = fs::remove_dir_all(&root);
    }

    /// The launcher is built but never run here — spawning it would open a Finder
    /// window during `cargo test`. What matters is that a path which is not a
    /// directory is refused instead of being handed to the OS.
    #[test]
    fn reveal_refuses_a_path_that_is_not_a_directory() {
        let file = std::env::temp_dir().join(format!("listenbli-file-{}", std::process::id()));
        fs::write(&file, b"x").unwrap();
        assert!(reveal_command(&file).is_err());

        let dir = std::env::temp_dir().join(format!("listenbli-dir-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(reveal_command(&dir).is_ok());

        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    /// `open_url` is the one place a URL from the network reaches a shell, so the
    /// filter is what stands between a `javascript:` link and the system opener.
    #[test]
    fn only_plain_https_urls_are_opened() {
        assert!(is_safe_https("https://www.bilibili.com/video/BV1fx411N7bU"));
        assert!(is_safe_https("https://example.com/a?b=c&d=e#f"));

        for rejected in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "http://www.bilibili.com/video/BV1",
            "https://example.com/$(rm -rf ~)",
            "https://example.com/`id`",
            "https://example.com/\"quoted\"",
            "https://",
        ] {
            assert!(!is_safe_https(rejected), "{rejected} should be refused");
            assert!(
                open_url(rejected).is_err(),
                "{rejected} should not reach the opener"
            );
        }
    }

    /// The window icon is the artwork embedded at build time. If the asset moves
    /// or stops being a PNG, `app_icon` returns `None` and eframe silently puts
    /// its own placeholder on the taskbar — so check the decode.
    #[test]
    fn the_embedded_app_icon_decodes() {
        let icon = app_icon().expect("assets/icon.png should decode as an icon");
        assert_eq!(
            icon.width, icon.height,
            "the app icon should be square, got {}x{}",
            icon.width, icon.height
        );
        assert!(icon.width >= 256, "the icon is too small: {}", icon.width);
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4,
            "RGBA buffer length must match the icon size"
        );
    }
}

//! Embed `assets/icon.ico` and the crate's version metadata as the Windows
//! executable's resources.
//!
//! The window icon is set at runtime (`platform::app_icon`, passed to eframe), but
//! Explorer, "Send to" and the shortcut-creator read the icon out of the PE
//! resource section instead. Without this an installed listenBli.exe shows the
//! generic executable icon, no matter what the window does.
//!
//! Only the Windows target needs it. We look for a resource compiler on PATH —
//! `rc.exe` from the Windows SDK, `llvm-rc` from an LLVM toolchain, or GNU
//! `windres` — and if none is found we warn and carry on: a cross type-check
//! (`cargo check --target x86_64-pc-windows-msvc` from macOS or Linux) should
//! still succeed, it just cannot produce the icon.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ico = root.join("assets").join("icon.ico");
    if !ico.exists() {
        println!(
            "cargo:warning={} 不存在，跳过 Windows 图标资源",
            ico.display()
        );
        return;
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    // The resource script is generated so the icon path can be absolute: neither
    // rc.exe nor windres resolves a relative .ico against the script's directory.
    let script = out.join("listenbli.rc");
    let contents = format!("IDI_ICON1 ICON \"{}\"\n{}", ico.display(), version_info());
    if let Err(err) = std::fs::write(&script, contents) {
        println!("cargo:warning=写入 {script:?} 失败：{err}");
        return;
    }
    let resource = out.join("listenbli.res");

    // `(program, args)`: rc.exe and llvm-rc share /fo, windres takes -i/-o.
    let attempts: [(&str, Vec<String>); 4] = [
        (
            "rc",
            vec![
                "/nologo".into(),
                "/fo".into(),
                path(&resource),
                path(&script),
            ],
        ),
        (
            "llvm-rc",
            vec!["/fo".into(), path(&resource), path(&script)],
        ),
        (
            "windres",
            vec![
                "-i".into(),
                path(&script),
                "-o".into(),
                path(&resource),
                "--output-format=coff".into(),
            ],
        ),
        (
            "x86_64-w64-mingw32-windres",
            vec![
                "-i".into(),
                path(&script),
                "-o".into(),
                path(&resource),
                "--output-format=coff".into(),
            ],
        ),
    ];

    let mut errors = Vec::new();
    for (program, args) in attempts {
        match Command::new(program).args(&args).output() {
            Ok(output) if output.status.success() && resource.exists() => {
                println!("cargo:rustc-link-arg-bins={}", path(&resource));
                return;
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                errors.push(format!("{program}: {}", stderr.trim()));
            }
            Err(err) => errors.push(format!("{program}: {err}")),
        }
    }

    println!(
        "cargo:warning=没找到可用的资源编译器（rc.exe/llvm-rc/windres），\
         exe 图标资源不会嵌入：{}",
        errors.join("；")
    );
}

/// The `VERSIONINFO` block behind 属性 → 详细信息.
///
/// Without it the exe carries no product name, version or copyright at all. The
/// values come from `CARGO_PKG_*`, which cargo exports to build scripts, so the
/// crate stays the single source of truth — the same fields the macOS bundle
/// fills into `Info.plist` from `Cargo.toml`.
///
/// The strings are deliberately ASCII: rc.exe reads the script in the system
/// code page unless it carries a BOM, so a middle dot here would turn into
/// mojibake (or a compile error) depending on the machine's locale.
fn version_info() -> String {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let description = std::env::var("CARGO_PKG_DESCRIPTION").unwrap_or_default();
    let repository = std::env::var("CARGO_PKG_REPOSITORY").unwrap_or_default();
    let copyright = if repository.is_empty() {
        "MIT licensed".to_owned()
    } else {
        format!("MIT licensed - {repository}")
    };
    // FILEVERSION wants four numbers, so "0.1.0" becomes "0,1,0,0".
    let mut parts: Vec<u16> = version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    parts.resize(4, 0);
    let numeric = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);

    format!(
        "\
1 VERSIONINFO\n\
FILEVERSION {numeric}\n\
PRODUCTVERSION {numeric}\n\
FILEOS 0x40004L\n\
FILETYPE 0x1L\n\
BEGIN\n\
  BLOCK \"StringFileInfo\"\n\
  BEGIN\n\
    BLOCK \"080404B0\"\n\
    BEGIN\n\
      VALUE \"FileDescription\", \"{description}\"\n\
      VALUE \"FileVersion\", \"{version}\"\n\
      VALUE \"LegalCopyright\", \"{copyright}\"\n\
      VALUE \"OriginalFilename\", \"listenbli.exe\"\n\
      VALUE \"ProductName\", \"listenBli\"\n\
      VALUE \"ProductVersion\", \"{version}\"\n\
    END\n\
  END\n\
  BLOCK \"VarFileInfo\"\n\
  BEGIN\n\
    VALUE \"Translation\", 0x0804, 1200\n\
  END\n\
END\n"
    )
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

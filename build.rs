//! Embed `assets/icon.ico` as the Windows executable's icon resource.
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
    if let Err(err) = std::fs::write(&script, format!("IDI_ICON1 ICON \"{}\"\n", ico.display())) {
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

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

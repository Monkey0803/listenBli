// Hide the console window on Windows release builds. On every other platform
// (and in debug builds) this attribute is inert.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use listenbli::app::App;
use listenbli::config::Config;
use listenbli::platform;
use listenbli::ui::theme;

fn main() -> eframe::Result<()> {
    let config = Config::load();
    // Resolve the CJK font before the window opens so the first frame is already
    // readable.
    let font_path = platform::resolve_cjk_font(&config.cjk_font, config.cjk_font_path.as_deref());

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([960.0, 640.0])
        .with_title("listenBli · 哔哩哔哩音乐");

    // macOS takes a bundle's icon from `Contents/Resources/ListenBli.icns`, but
    // eframe calls `setApplicationIconImage` at startup with its own placeholder
    // whenever no icon is given, and that runtime image is what the Dock draws
    // while the app runs. Finder keeps showing the .icns, so the two disagreed:
    // pink squircle in Finder, eframe's black hexagon in the Dock. Handing eframe
    // an empty icon is its signal to leave the icon alone, which leaves the
    // bundle (and the system's own icon treatment) in charge.
    #[cfg(target_os = "macos")]
    let viewport = viewport.with_icon(egui::IconData::default());

    // Everywhere else there is no bundle icon to defer to, so eframe gets the
    // real artwork: otherwise the same placeholder would show up on the Windows
    // taskbar and in alt-tab. The executable's own icon comes from the resource
    // `build.rs` embeds.
    #[cfg(not(target_os = "macos"))]
    let viewport = match platform::app_icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "listenbli",
        options,
        Box::new(move |cc| {
            // Registers the UI, lyric-serif and monospaced families; the font
            // set is rebuilt from the settings sheet when the choice changes.
            theme::install_fonts(&cc.egui_ctx, font_path.as_deref());
            Ok(Box::new(App::new(config)))
        }),
    )
}

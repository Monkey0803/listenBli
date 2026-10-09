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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title("listenBli · 哔哩哔哩音乐"),
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

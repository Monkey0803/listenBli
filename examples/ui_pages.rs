//! Render every page headlessly and write one PNG each.
//!
//! `ui_snapshot` needs a real window, and a locked screen or a CI box has none —
//! the screenshot request simply never comes back and the app repaints forever.
//! This draws the same shell through an offscreen wgpu renderer instead, so the
//! per-page review does not depend on the display being awake.
//!
//! ```bash
//! cargo run --example ui_pages -- /tmp/listenbli-pages
//! ```
//!
//! It composes `App::ui_shell`, the same call `eframe::App::ui` makes, so what
//! lands in the PNG is the interface and not a copy of it.

mod common;

use egui::Vec2;
use egui_kittest::Harness;
use listenbli::app::App;
use listenbli::config::Config;
use listenbli::platform;
use listenbli::ui::theme::{self, Theme};

/// What to draw: the file's name, the page to ask for, and the library behind it.
///
/// `seed` is the variant `common::seed_library` understands; an empty string means
/// an untouched library, which is the first-run state and the one where a layout
/// mistake has nothing to hide behind.
const SCENARIOS: [(&str, &str, &str); 17] = [
    ("library", "library", "library"),
    ("playlists", "playlists", "library"),
    ("recent", "recent", "library"),
    ("offline", "offline", "library"),
    ("search", "search", "library"),
    ("sources", "sources", "library"),
    // 来源与同步 with a hand-declared source that has no implementation behind it.
    ("declared", "sources", "declaration"),
    // 来源详情 is a sheet rather than a page, and the design's one place a
    // platform is allowed to show — so it is worth a picture of its own.
    ("source", "source", "library"),
    // The state that takes a failing platform to reach: one copy of a song is
    // gone and another is playing in its place.
    // The declaration form, and the state a declaration is in before anything
    // can be fetched from it.
    ("connect", "connect", "library"),
    ("fallback", "library", "fallback"),
    ("fallback-source", "source", "fallback"),
    // And the empty library, one picture per page.
    ("empty-library", "library", ""),
    ("empty-playlists", "playlists", ""),
    ("empty-recent", "recent", ""),
    ("empty-offline", "offline", ""),
    ("empty-search", "search", ""),
    ("empty-sources", "sources", ""),
];

fn main() {
    common::isolate_home();
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/listenbli-pages".to_owned());
    std::fs::create_dir_all(&out).expect("a place to write the images");

    for (name, page, seed) in SCENARIOS {
        let config = Config::load();
        let theme = Theme::from_config(&config);
        let cjk = platform::resolve_cjk_font(&config.cjk_font, config.cjk_font_path.as_deref());
        let mut app = App::new_silent(config);
        if seed.is_empty() {
            // The scenarios share a home, and a seeded one saves its library, so
            // "empty" has to be said rather than assumed.
            *app.library_mut() = listenbli::library::LocalLibrary::default();
        } else {
            common::seed_library(&mut app, seed);
        }
        // After seeding: the seed puts search results together, which moves the
        // app to 搜索, so the page has to be asked for last.
        app.open_overlay(page);
        if name.ends_with("-source") {
            // The sheet floats over whatever page is behind it.
            app.open_overlay("source");
        }

        let mut harness = Harness::builder()
            .with_size(Vec2::new(1440.0, 900.0))
            .wgpu()
            // The harness makes its own egui context and renders a frame the
            // moment it is built, and this app's theme defines a `lyric` family
            // that nothing else binds — a missing family is a panic, not a
            // fallback. `set_fonts` only takes effect on the *next* pass, so the
            // first pass installs them, the second lets egui apply them, and the
            // shell draws from the third on.
            .build_ui_state(
                move |ui, pass: &mut u8| {
                    *pass += 1;
                    match *pass {
                        1 => {
                            theme::install_fonts(ui.ctx(), cjk.as_deref());
                            theme::install_style(ui.ctx(), &theme);
                        }
                        2 => {}
                        _ => {
                            app.ui_shell(ui);
                            app.ui_overlays(ui.ctx());
                        }
                    }
                },
                0,
            );
        // Fonts and layout settle over a few frames, for the same reason
        // `ui_snapshot` waits before asking for its capture.
        for _ in 0..4 {
            harness.run();
        }

        let image = harness.render().expect("the offscreen renderer");
        let path = format!("{out}/{name}.png");
        image.save(&path).expect("writing the image");
        eprintln!("wrote {path}");
    }
}

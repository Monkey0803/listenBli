//! Render the UI once and write a PNG, then exit.
//!
//! A hand-painted egui surface cannot be checked by a unit test, and the window
//! is not always reachable (CI, headless boxes, a busy desktop). This example
//! mounts the real `App`, waits a few frames for fonts and layout to settle, and
//! asks the backend for a screenshot:
//!
//! ```bash
//! cargo run --example ui_snapshot -- /tmp/listenbli.png
//! # or a specific window size:
//! cargo run --example ui_snapshot -- /tmp/small.png 960 640
//! ```
//!
//! Pass `LISTENBLI_OPEN=settings` (or `queue`) to capture an overlay instead of
//! the default state.

use std::path::PathBuf;

use listenbli::app::App;
use listenbli::config::Config;
use listenbli::platform;
use listenbli::ui::theme;

/// How many frames to let the UI settle before asking for the capture.
const SETTLE_FRAMES: u32 = 30;

struct Snapshot {
    app: App,
    output: PathBuf,
    frame: u32,
    requested: bool,
}

impl Snapshot {
    /// Applies `LISTENBLI_OPEN` once, after the first frame has run.
    fn open_requested_overlay(&mut self, ctx: &egui::Context) {
        if let Ok(name) = std::env::var("LISTENBLI_OPEN") {
            self.app.open_overlay(&name);
            ctx.request_repaint();
        }
    }
}

impl eframe::App for Snapshot {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(image) = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        }) {
            let mut buffer = image::RgbaImage::new(image.size[0] as u32, image.size[1] as u32);
            for (index, pixel) in image.pixels.iter().enumerate() {
                buffer.put_pixel(
                    (index % image.size[0]) as u32,
                    (index / image.size[0]) as u32,
                    image::Rgba(pixel.to_array()),
                );
            }
            match buffer.save(&self.output) {
                Ok(()) => eprintln!("snapshot written to {}", self.output.display()),
                Err(err) => eprintln!("snapshot failed: {err}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.app.ui(ui, frame);

        if self.frame == 1 {
            self.open_requested_overlay(&ctx);
        }
        if self.frame >= SETTLE_FRAMES && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        self.frame += 1;
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().unwrap_or_else(|| "listenbli.png".to_owned()));
    let width: f32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1280.0);
    let height: f32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(820.0);

    let config = Config::load();
    let font = platform::resolve_cjk_font(&config.cjk_font, config.cjk_font_path.as_deref());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([width, height])
            .with_title("listenBli snapshot"),
        ..Default::default()
    };

    eframe::run_native(
        "listenbli-snapshot",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx, font.as_deref());
            Ok(Box::new(Snapshot {
                app: App::new(config),
                output,
                frame: 0,
                requested: false,
            }))
        }),
    )
}

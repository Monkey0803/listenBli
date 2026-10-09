//! Design tokens for the "深夜电台 / Midnight Broadcast" look.
//!
//! Everything the UI paints itself goes through these constants, so the palette
//! lives in exactly one place. The accent colour is the only token the user can
//! change; the four derived values (`deep`/`soft`/`ink`/`dim`) follow it.

use std::path::Path;
use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Mesh, Pos2,
    Rect, Shape, Stroke, Vec2,
};

use crate::platform;

// ---------------------------------------------------------------------------
// palette
// ---------------------------------------------------------------------------

/// Layered charcoal, coldest at the back.
pub const INK_0: Color32 = Color32::from_rgb(0x06, 0x06, 0x0a);
pub const INK_1: Color32 = Color32::from_rgb(0x0c, 0x0c, 0x11);
pub const INK_2: Color32 = Color32::from_rgb(0x11, 0x11, 0x17);
pub const INK_3: Color32 = Color32::from_rgb(0x17, 0x17, 0x1f);
pub const INK_4: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x28);
pub const INK_5: Color32 = Color32::from_rgb(0x28, 0x28, 0x33);

pub const LINE: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 17);
pub const LINE_2: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 29);
pub const LINE_3: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 46);

pub const FG: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 241);
pub const FG_2: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 168);
pub const FG_3: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 122);
pub const FG_4: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 82);

/// Bilibili blue — translations and quality badges.
pub const BLUE: Color32 = Color32::from_rgb(0x35, 0xb6, 0xe4);
pub const BLUE_INK: Color32 = Color32::from_rgb(0x8f, 0xd6, 0xf2);
pub const GOLD: Color32 = Color32::from_rgb(0xe8, 0xc0, 0x7d);
pub const OK: Color32 = Color32::from_rgb(0x4f, 0xd3, 0x9b);
pub const WARN: Color32 = Color32::from_rgb(0xf0, 0xa5, 0x5c);
pub const ERR: Color32 = Color32::from_rgb(0xfb, 0x71, 0x85);

// radii
pub const R_SM: f32 = 8.0;
pub const R_MD: f32 = 10.0;
pub const R_LG: f32 = 14.0;
pub const R_XL: f32 = 18.0;

/// Height of the top bar, player bar and status bar. The central row takes the
/// rest, mirroring the design's `58px / 1fr / auto / 30px` grid.
pub const TOP_BAR_H: f32 = 58.0;
pub const STATUS_BAR_H: f32 = 30.0;

pub fn hairline() -> Stroke {
    Stroke::new(1.0, LINE)
}

pub fn hairline_2() -> Stroke {
    Stroke::new(1.0, LINE_2)
}

/// A translucent white overlay — the design's `rgba(255,255,255,a)`.
pub fn white(alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8)
}

/// Renders `color` at `alpha` (0..=1).
pub fn fade(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8)
}

/// Linear interpolation in gamma space — good enough for the two-stop
/// gradients the design uses, and much cheaper than an oklab conversion.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

// ---------------------------------------------------------------------------
// accent
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    pub id: &'static str,
    pub name: &'static str,
    pub accent: Color32,
    pub deep: Color32,
    pub soft: Color32,
    pub ink: Color32,
}

pub const ACCENTS: [Accent; 3] = [
    Accent {
        id: "pink",
        name: "信号粉",
        accent: Color32::from_rgb(0xfb, 0x72, 0x99),
        deep: Color32::from_rgb(0xd9, 0x57, 0x8a),
        soft: Color32::from_rgb(0xff, 0x9d, 0xb8),
        ink: Color32::from_rgb(0xff, 0xc8, 0xd8),
    },
    Accent {
        id: "cyan",
        name: "霓虹青",
        accent: Color32::from_rgb(0x35, 0xb6, 0xe4),
        deep: Color32::from_rgb(0x1d, 0x86, 0xad),
        soft: Color32::from_rgb(0x7f, 0xd6, 0xf0),
        ink: Color32::from_rgb(0xb7, 0xe9, 0xf8),
    },
    Accent {
        id: "violet",
        name: "夜紫",
        accent: Color32::from_rgb(0xa7, 0x8b, 0xfa),
        deep: Color32::from_rgb(0x7c, 0x5c, 0xf0),
        soft: Color32::from_rgb(0xc4, 0xb0, 0xfd),
        ink: Color32::from_rgb(0xdd, 0xd2, 0xfe),
    },
];

pub fn accent_by_id(id: &str) -> Accent {
    ACCENTS
        .iter()
        .copied()
        .find(|a| a.id == id)
        .unwrap_or(ACCENTS[0])
}

impl Accent {
    /// The accent at `alpha` — used for the 22% "dim" fills and glows.
    pub fn dim(self) -> Color32 {
        fade(self.accent, 0.22)
    }

    pub fn glow(self) -> Color32 {
        fade(self.accent, 0.55)
    }

    /// A gradient brush for the play button and other accent surfaces.
    pub fn ramp(self) -> (Color32, Color32) {
        (self.soft, self.accent)
    }
}

// ---------------------------------------------------------------------------
// theme
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub accent: Accent,
    pub lyric_size: f32,
    pub row_h: f32,
}

impl Theme {
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            accent: accent_by_id(&config.accent),
            lyric_size: config.lyric_size.clamp(13.0, 22.0),
            row_h: if config.density == "compact" {
                52.0
            } else {
                62.0
            },
        }
    }

    pub fn compact(&self) -> bool {
        self.row_h < 60.0
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: ACCENTS[0],
            lyric_size: 15.5,
            row_h: 62.0,
        }
    }
}

// ---------------------------------------------------------------------------
// fonts
// ---------------------------------------------------------------------------

/// Family used for the lyric body (a CJK serif, per the design).
pub const LYRIC_FAMILY: &str = "lyric";

pub fn lyric_family() -> FontFamily {
    FontFamily::Name(LYRIC_FAMILY.into())
}

pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn mono_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn lyric_font(size: f32) -> FontId {
    FontId::new(size, lyric_family())
}

/// Identifies the current font selection so a settings change can be detected.
pub fn fonts_key(config: &crate::config::Config) -> String {
    format!(
        "{}|{}",
        config.cjk_font,
        config
            .cjk_font_path
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    )
}

fn read(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Contribute the system CJK face, a lyric serif and a monospaced face to egui's
/// font set. egui ships no CJK glyphs, so without this every Chinese label
/// renders as tofu.
pub fn install_fonts(ctx: &Context, cjk: Option<&Path>) {
    let mut fonts = FontDefinitions::default();
    // egui's own fallbacks, so the faces below can defer to them.
    let proportional_defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let monospace_defaults = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let mut chain: Vec<String> = Vec::new();

    if let Some(bytes) = cjk.and_then(read) {
        fonts
            .font_data
            .insert("cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
        // PingFang carries decent Latin glyphs too, so it can lead the UI font
        // stack: one face, consistent metrics across scripts.
        chain.push("cjk".to_owned());
    } else {
        eprintln!(
            "未找到可用的中文字体，界面中的中文将无法显示。\
             请在设置里选择字体，或在配置文件的 \"cjk_font_path\" 中指定字体文件路径。"
        );
    }

    let mut proportional = chain.clone();
    proportional.extend(proportional_defaults.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);

    // Lyrics read best in a serif; on macOS that is Songti.
    let mut lyric = Vec::new();
    if let Some(bytes) = platform::lyric_font().as_deref().and_then(read) {
        fonts.font_data.insert(
            "lyric-serif".to_owned(),
            Arc::new(FontData::from_owned(bytes)),
        );
        lyric.push("lyric-serif".to_owned());
    }
    lyric.extend(chain.iter().cloned());
    lyric.extend(proportional_defaults);
    fonts.families.insert(lyric_family(), lyric);

    // SF Mono for durations, UIDs and paths.
    let mut monospace = Vec::new();
    if let Some(bytes) = platform::mono_font().as_deref().and_then(read) {
        fonts
            .font_data
            .insert("ui-mono".to_owned(), Arc::new(FontData::from_owned(bytes)));
        monospace.push("ui-mono".to_owned());
    }
    monospace.extend(chain);
    monospace.extend(monospace_defaults);
    fonts.families.insert(FontFamily::Monospace, monospace);

    ctx.set_fonts(fonts);
}

/// Global widget style. Only affects the handful of stock widgets still in use
/// (the search field, sliders and scroll bars); the rest is hand-painted.
pub fn install_style(ctx: &Context, theme: &Theme) {
    ctx.all_styles_mut(|style| {
        let mut visuals = egui::Visuals::dark();

        visuals.panel_fill = INK_1;
        visuals.window_fill = INK_2;
        visuals.window_stroke = Stroke::new(1.0, LINE_2);
        visuals.window_corner_radius = CornerRadius::same(R_LG as u8);
        visuals.extreme_bg_color = Color32::from_black_alpha(110);
        visuals.faint_bg_color = INK_3;
        visuals.override_text_color = Some(FG);
        visuals.selection.bg_fill = theme.accent.dim();
        visuals.selection.stroke = Stroke::new(1.0, theme.accent.accent);
        visuals.hyperlink_color = theme.accent.accent;
        visuals.text_cursor.stroke = Stroke::new(1.4, theme.accent.accent);

        {
            let widget = &mut visuals.widgets;
            widget.noninteractive.bg_fill = INK_2;
            widget.noninteractive.weak_bg_fill = INK_2;
            widget.noninteractive.bg_stroke = hairline();
            widget.noninteractive.fg_stroke = Stroke::new(1.0, FG_2);
            widget.noninteractive.corner_radius = CornerRadius::same(R_SM as u8);

            widget.inactive.bg_fill = white(0.09);
            widget.inactive.weak_bg_fill = white(0.045);
            widget.inactive.bg_stroke = hairline();
            widget.inactive.fg_stroke = Stroke::new(1.0, FG_2);
            widget.inactive.corner_radius = CornerRadius::same(R_SM as u8);

            widget.hovered.bg_fill = white(0.17);
            widget.hovered.weak_bg_fill = white(0.075);
            widget.hovered.bg_stroke = hairline_2();
            widget.hovered.fg_stroke = Stroke::new(1.0, FG);
            widget.hovered.corner_radius = CornerRadius::same(R_SM as u8);

            widget.active.bg_fill = fade(theme.accent.accent, 0.3);
            widget.active.weak_bg_fill = fade(theme.accent.accent, 0.18);
            widget.active.bg_stroke = Stroke::new(1.0, fade(theme.accent.accent, 0.4));
            widget.active.fg_stroke = Stroke::new(1.0, FG);
            widget.active.corner_radius = CornerRadius::same(R_SM as u8);

            widget.open.bg_fill = theme.accent.dim();
            widget.open.weak_bg_fill = theme.accent.dim();
            widget.open.bg_stroke = Stroke::new(1.0, fade(theme.accent.accent, 0.35));
            widget.open.fg_stroke = Stroke::new(1.0, theme.accent.ink);
        }

        style.visuals = visuals;
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.spacing.interact_size = Vec2::new(24.0, 24.0);
        style.spacing.slider_width = 120.0;
        style.spacing.scroll.bar_width = 9.0;
        style.spacing.scroll.bar_inner_margin = 3.0;
        style.spacing.scroll.bar_outer_margin = 2.0;
        style.spacing.scroll.floating = true;

        use egui::TextStyle as TS;
        for (style_name, size) in [
            (TS::Small, 11.5),
            (TS::Body, 13.0),
            (TS::Button, 13.0),
            (TS::Heading, 17.0),
            (TS::Monospace, 11.5),
        ] {
            style
                .text_styles
                .insert(style_name, FontId::new(size, FontFamily::Proportional));
        }
    });
}

// ---------------------------------------------------------------------------
// painting helpers
// ---------------------------------------------------------------------------

/// The boundary of a rounded rectangle, walked clockwise from the top-left
/// corner's arc. Used to build gradient meshes with rounded corners.
pub fn rounded_rect_points(rect: Rect, radius: f32, segments: usize) -> Vec<Pos2> {
    let r = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    let mut points = Vec::with_capacity(segments * 4 + 4);
    let corners = [
        (rect.right() - r, rect.top() + r, -90.0f32), // top-right
        (rect.right() - r, rect.bottom() - r, 0.0),   // bottom-right
        (rect.left() + r, rect.bottom() - r, 90.0),   // bottom-left
        (rect.left() + r, rect.top() + r, 180.0),     // top-left
    ];
    for (cx, cy, start) in corners {
        for step in 0..=segments {
            let angle = (start + 90.0 * step as f32 / segments as f32).to_radians();
            points.push(Pos2::new(cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    if r <= 0.0 {
        return vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ];
    }
    points
}

/// A two-stop gradient clipped to a rounded rectangle.
///
/// epaint has no gradient primitives, so the rect is tessellated as a fan from
/// its centre: every boundary vertex takes the gradient colour at its own
/// position, and the centre takes the midpoint colour. The result is exact along
/// the edges and visually indistinguishable from a linear gradient inside.
pub fn gradient_rounded_rect(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    from: Color32,
    to: Color32,
    axis: Vec2,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    // Projection of a point onto the gradient axis, normalised to 0..=1.
    let span = rect.size().dot(axis.normalized());
    let project = |p: Pos2| {
        if span.abs() < f32::EPSILON {
            0.5
        } else {
            ((p - rect.min).dot(axis.normalized()) / span).clamp(0.0, 1.0)
        }
    };

    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), mix(from, to, project(rect.center())));
    // A full circle needs more segments than a 8px card corner; scale with the
    // radius so the play button does not read as a polygon.
    let segments = ((radius / 2.5).ceil() as usize).clamp(4, 24);
    let points = rounded_rect_points(rect, radius, segments);
    for point in &points {
        mesh.colored_vertex(*point, mix(from, to, project(*point)));
    }
    let count = points.len() as u32;
    for i in 0..count {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % count);
    }
    painter.add(Shape::mesh(mesh));
}

/// egui cannot blur, so a "bloom" is a single radial mesh: the centre carries
/// the full colour and the rim is transparent, which interpolates smoothly
/// instead of the banding a stack of circles would produce.
pub fn bloom(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, alpha: f32) {
    const STEPS: u32 = 56;
    if radius <= 0.0 || alpha <= 0.0 {
        return;
    }
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, fade(color, alpha));
    for step in 0..STEPS {
        let angle = step as f32 / STEPS as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(center + Vec2::angled(angle) * radius, fade(color, 0.0));
    }
    for step in 0..STEPS {
        mesh.add_triangle(0, 1 + step, 1 + (step + 1) % STEPS);
    }
    painter.add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_lookup_falls_back_to_the_default() {
        assert_eq!(accent_by_id("pink").id, "pink");
        assert_eq!(accent_by_id("does-not-exist").id, ACCENTS[0].id);
    }

    #[test]
    fn mix_hits_both_ends() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(255, 128, 64);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 99.0), b);
    }

    #[test]
    fn rounded_rect_points_stay_inside_the_rect() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(40.0, 30.0));
        for point in rounded_rect_points(rect, 8.0, 4) {
            assert!(rect.contains(point), "{point:?} escaped {rect:?}");
        }
    }
}

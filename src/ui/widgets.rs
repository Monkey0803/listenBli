//! Reusable hand-painted controls: everything in the design that egui has no
//! stock widget for (pills, chips, toggles, scrubbers, cover art, empty states).

use std::sync::Arc;

use egui::{
    Align, Align2, Color32, CursorIcon, FontId, Galley, Pos2, Rect, Response, Sense, Stroke,
    TextureHandle, Ui, Vec2,
};

use super::icons::Icon;
use super::theme::{self, mono_font, ui_font, Accent};
use crate::api::models::{AudioQuality, Track};

// ---------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------

/// Text measurement. `Borrow<FontId>` lets callers pass an owned `FontId` or a
/// reference, which keeps the many `measure(ui, x, &font)` call sites tidy.
pub fn measure(ui: &Ui, text: &str, font: impl std::borrow::Borrow<FontId>) -> Vec2 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font.borrow().clone(), Color32::WHITE)
        .size()
}

/// One line of text, elided with `…` when it does not fit `width`.
pub fn elided(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = width.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.painter().layout_job(job)
}

/// Vertical centre of a galley's glyph ink, measured from the galley's top.
///
/// egui anchors text by the font's ascent/descent box, which on CJK faces sits
/// a couple of pixels above the glyphs themselves — enough to make a label look
/// visibly high next to the icon drawn beside it. Measuring the tessellated
/// quads gives the ink, which is what the eye actually centres on.
pub fn ink_center(galley: &Galley) -> f32 {
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for placed in &galley.rows {
        let bounds = placed.row.visuals.mesh_bounds;
        // `Rect::NOTHING` (an empty row) compares as max <= min.
        if bounds.max.y <= bounds.min.y {
            continue;
        }
        top = top.min(bounds.min.y + placed.pos.y);
        bottom = bottom.max(bounds.max.y + placed.pos.y);
    }
    if top.is_finite() && bottom > top {
        (top + bottom) * 0.5
    } else {
        galley.size().y * 0.5
    }
}

/// A constant nudge that centres UI text on a fixed-height field's centre line.
///
/// `TextEdit` centres the font's ascent/descent box, which on CJK faces leaves
/// the glyphs a couple of pixels high. The value is measured from a fixed
/// sample rather than the live text, so the line never shifts while typing.
pub fn field_text_offset(painter: &egui::Painter, font: &FontId) -> f32 {
    let sample = painter.layout_no_wrap("测Ag".to_owned(), font.clone(), Color32::WHITE);
    sample.size().y * 0.5 - ink_center(&sample)
}

/// Single-line layout for a text field, nudged so the glyph ink — not the
/// font's box — lands on the field's centre line.
pub fn field_galley(
    ui: &Ui,
    text: &str,
    font: &FontId,
    color: Color32,
    offset: f32,
    wrap_width: f32,
) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font.clone(), color, wrap_width);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let mut galley = ui.painter().layout_job(job);
    if offset.abs() > 0.05 {
        for row in Arc::make_mut(&mut galley).rows.iter_mut() {
            row.pos.y += offset;
        }
    }
    galley
}

/// Ink centre of the first row only, relative to the galley's top — the anchor
/// an icon beside a wrapped paragraph wants.
pub fn first_row_ink_center(galley: &Galley) -> f32 {
    match galley.rows.first() {
        Some(placed) => {
            let bounds = placed.row.visuals.mesh_bounds;
            if bounds.max.y > bounds.min.y {
                placed.pos.y + (bounds.min.y + bounds.max.y) * 0.5
            } else {
                galley
                    .rows
                    .first()
                    .map(|r| r.pos.y + r.row.size.y * 0.5)
                    .unwrap_or(galley.size().y * 0.5)
            }
        }
        None => galley.size().y * 0.5,
    }
}

/// Draws one line of text; `LEFT_CENTER` and `CENTER_CENTER` centre the glyph
/// ink, so the line lines up with shapes drawn at the same `y`.
pub fn paint_label(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    text: impl AsRef<str>,
    font: FontId,
    color: Color32,
) -> Rect {
    let text = text.as_ref();
    if text.is_empty() {
        return Rect::NOTHING;
    }
    let galley = painter.layout_no_wrap(text.to_owned(), font, color);
    let rect = anchor.anchor_size(pos, galley.size());
    let top_left = if anchor.y() == Align::Center {
        Pos2::new(rect.min.x, pos.y - ink_center(&galley))
    } else {
        rect.min
    };
    painter.galley(top_left, galley, color);
    rect
}

/// Paints an already-laid-out galley with its ink vertically centred on
/// `pos.y` and its left edge at `pos.x`.
pub fn paint_galley_centred(
    painter: &egui::Painter,
    pos: Pos2,
    galley: Arc<Galley>,
    color: Color32,
) {
    painter.galley(Pos2::new(pos.x, pos.y - ink_center(&galley)), galley, color);
}

pub fn label_at(ui: &Ui, pos: Pos2, anchor: Align2, text: &str, font: FontId, color: Color32) {
    paint_label(ui.painter(), pos, anchor, text, font, color);
}

/// egui ships no bold face; a half-pixel offset doubles the stems convincingly
/// at UI sizes, and costs one extra glyph run.
pub fn label_at_bold(ui: &Ui, pos: Pos2, anchor: Align2, text: &str, font: FontId, color: Color32) {
    if text.is_empty() {
        return;
    }
    paint_label(ui.painter(), pos, anchor, text, font.clone(), color);
    paint_label(
        ui.painter(),
        pos + Vec2::new(0.45, 0.0),
        anchor,
        text,
        font,
        color,
    );
}

/// A single elided line, left- or right-aligned inside `rect`, vertically
/// centred.
pub fn clipped_line(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32, right: bool) {
    let galley = elided(ui, text, font, color, rect.width());
    let x = if right {
        rect.right() - galley.size().x
    } else {
        rect.left()
    };
    paint_galley_centred(ui.painter(), Pos2::new(x, rect.center().y), galley, color);
}

/// The design's `.field__hint` leading: 11px text on a 1.6 line height.
pub(crate) const HINT_LEADING: f32 = 1.6;

/// `<text>` laid out wrapped, with the leading the design asks for.
///
/// epaint spaces its rows from the font's own metrics (a bit over 1.1× here),
/// which is tighter than every multi-line paragraph in the design
/// (`.field__hint` 1.6, `.modal__desc` 1.7, `.state__desc` 1.75). Without an
/// explicit `TextFormat::line_height` two lines of a hint read as one block.
/// `leading` is a multiple of the font size.
fn wrapped(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    leading: f32,
) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font.clone(),
            color,
            line_height: Some(font.size * leading),
            ..Default::default()
        },
    );
    ui.painter().layout_job(job)
}

/// A left-aligned, wrapped hint line — the design's `.field__hint`.
pub(crate) fn hint(ui: &mut Ui, text: &str, size: f32, color: Color32) {
    let width = ui.available_width();
    let galley = wrapped(ui, text, ui_font(size), color, width, HINT_LEADING);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

/// A wrapped paragraph, centred inside the available width.
///
/// `leading` is a multiple of the font size, as in the design's
/// `.state__desc` (1.75) and `.modal__desc` (1.7).
pub fn centred_paragraph(
    ui: &mut Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
    leading: f32,
) {
    let width = max_width.min(ui.available_width() - 32.0).max(80.0);
    let galley = wrapped(ui, text, font, color, width, leading);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

// ---------------------------------------------------------------------------
// surfaces
// ---------------------------------------------------------------------------

/// Paints a fully-rounded pill.
pub fn paint_pill(ui: &Ui, rect: Rect, fill: Color32, stroke: Option<Stroke>) {
    let radius = rect.height() * 0.5;
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if let Some(stroke) = stroke {
        ui.painter()
            .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    }
}

// ---------------------------------------------------------------------------
// controls
// ---------------------------------------------------------------------------

/// A square icon button that allocates its own space.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    size: f32,
    active: bool,
    accent: Accent,
    tip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    paint_icon_button(ui, rect, icon, active, accent, response.hovered());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    }
}

/// The same button painted into an existing rect, for rows laid out by hand —
/// where a second `allocate` would push the surrounding layout around.
pub fn icon_button_at(
    ui: &mut Ui,
    id: egui::Id,
    rect: Rect,
    icon: Icon,
    active: bool,
    accent: Accent,
    tip: &str,
) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    paint_icon_button(ui, rect, icon, active, accent, response.hovered());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    }
}

fn paint_icon_button(ui: &Ui, rect: Rect, icon: Icon, active: bool, accent: Accent, hovered: bool) {
    let painter = ui.painter().clone();
    if active {
        painter.rect_filled(rect, theme::R_SM, accent.dim());
    } else if hovered {
        painter.rect_filled(rect, theme::R_SM, theme::INK_4);
    }
    let color = if active {
        accent.accent
    } else if hovered {
        theme::FG
    } else {
        theme::FG_2
    };
    let size = rect.width().min(rect.height());
    icon(&painter, rect.shrink(size * 0.22), color);
}

/// A rounded chip: optional leading icon, label, optional trailing count.
pub fn chip(
    ui: &mut Ui,
    label: &str,
    icon: Option<Icon>,
    trailing: Option<&str>,
    active: bool,
    accent: Accent,
) -> Response {
    let font = ui_font(12.0);
    let text_w = if label.is_empty() {
        0.0
    } else {
        measure(ui, label, &font).x
    };
    let icon_w = if icon.is_some() { 19.0 } else { 0.0 };
    let trailing_w = trailing
        .map(|t| measure(ui, t, mono_font(10.5)).x + 18.0)
        .unwrap_or(0.0);
    let width = text_w + icon_w + trailing_w + 22.0;

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered();

    let (fill, stroke, fg) = if active {
        (
            accent.dim(),
            Stroke::new(1.0, theme::fade(accent.accent, 0.4)),
            accent.ink,
        )
    } else {
        (
            theme::white(if hovered { 0.055 } else { 0.028 }),
            Stroke::new(1.0, if hovered { theme::LINE_2 } else { theme::LINE }),
            if hovered { theme::FG } else { theme::FG_2 },
        )
    };
    paint_pill(ui, rect, fill, Some(stroke));

    let mut cursor = rect.left() + 11.0;
    if let Some(icon) = icon {
        icon(
            &painter,
            Rect::from_min_size(Pos2::new(cursor, rect.center().y - 6.5), Vec2::splat(13.0)),
            fg,
        );
        cursor += 19.0;
    }
    if !label.is_empty() {
        paint_label(
            &painter,
            cursor_pos(cursor, rect),
            Align2::LEFT_CENTER,
            label,
            font,
            fg,
        );
    }
    if let Some(trailing) = trailing {
        let font = mono_font(10.5);
        let w = measure(ui, trailing, &font).x;
        let badge = Rect::from_min_size(
            Pos2::new(rect.right() - 10.0 - w - 6.0, rect.center().y - 7.5),
            Vec2::new(w + 12.0, 15.0),
        );
        painter.rect_filled(
            badge,
            7.5,
            if active {
                accent.dim()
            } else {
                theme::white(0.06)
            },
        );
        paint_label(
            &painter,
            badge.center(),
            Align2::CENTER_CENTER,
            trailing,
            font,
            if active { accent.ink } else { theme::FG_3 },
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

fn cursor_pos(x: f32, rect: Rect) -> Pos2 {
    Pos2::new(x, rect.center().y)
}

/// The iOS-style toggle from the settings sheet.
pub fn switch(ui: &mut Ui, checked: &mut bool, accent: Accent) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(34.0, 20.0), Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let painter = ui.painter().clone();
    let hovered = response.hovered();

    painter.rect_filled(
        rect,
        10.0,
        if *checked {
            accent.accent
        } else {
            theme::white(0.13)
        },
    );
    let knob = if *checked {
        Pos2::new(rect.right() - 10.0, rect.center().y)
    } else {
        Pos2::new(rect.left() + 10.0, rect.center().y)
    };
    if hovered {
        painter.rect_stroke(
            rect.expand(3.0),
            13.0,
            Stroke::new(3.0, theme::white(0.05)),
            egui::StrokeKind::Outside,
        );
    }
    painter.circle_filled(knob, 8.0, Color32::WHITE);

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

/// A small segmented control (settings: density, font choice).
pub fn segmented(ui: &mut Ui, options: &[(&str, &str)], selected: &mut String) -> bool {
    let font = ui_font(11.5);
    let total: f32 = options
        .iter()
        .map(|(_, label)| measure(ui, label, &font).x + 20.0)
        .sum::<f32>()
        + 6.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 30.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 9.0, theme::white(0.045));

    let mut changed = false;
    let mut cursor = rect.left() + 3.0;
    for (id, label) in options {
        let width = measure(ui, label, &font).x + 20.0;
        let cell = Rect::from_min_size(Pos2::new(cursor, rect.top() + 3.0), Vec2::new(width, 24.0));
        let response = ui.interact(cell, ui.id().with(("seg", id)), Sense::click());
        let on = selected == id;
        if on {
            painter.rect_filled(cell, 7.0, theme::INK_5);
        }
        paint_label(
            &painter,
            cell.center(),
            Align2::CENTER_CENTER,
            *label,
            font.clone(),
            if on {
                theme::FG
            } else if response.hovered() {
                theme::FG_2
            } else {
                theme::FG_3
            },
        );
        if response.clicked() && !on {
            *selected = (*id).to_owned();
            changed = true;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        cursor += width;
    }
    changed
}

/// A colour swatch for the accent picker.
pub fn swatch(ui: &mut Ui, accent: Accent, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let painter = ui.painter().clone();
    if selected {
        painter.circle_stroke(rect.center(), 12.5, Stroke::new(1.5, accent.accent));
    }
    let radius = if response.hovered() { 11.5 } else { 10.5 };
    theme::gradient_rounded_rect(
        &painter,
        Rect::from_center_size(rect.center(), Vec2::splat(radius * 2.0)),
        99.0,
        accent.soft,
        accent.accent,
        Vec2::new(0.6, 0.8),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

/// A horizontal range control, used for the lyric size.
pub fn range(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    width: f32,
    accent: Accent,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::click_and_drag());
    if let Some(pos) = response.interact_pointer_pos() {
        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        *value = range.start() + (range.end() - range.start()) * t;
    }
    let painter = ui.painter().clone();
    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 4.0));
    painter.rect_filled(track, 2.0, theme::white(0.13));
    let t = ((*value - range.start()) / (range.end() - range.start())).clamp(0.0, 1.0);
    let filled = Rect::from_min_size(track.min, Vec2::new(track.width() * t, track.height()));
    painter.rect_filled(filled, 2.0, accent.accent);
    painter.circle_filled(
        Pos2::new(track.left() + track.width() * t, track.center().y),
        6.5,
        Color32::WHITE,
    );
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

/// The seek/volume scrubber: a track, a buffered segment, a filled segment and
/// a knob that only appears on hover.
pub struct Scrub {
    pub response: Response,
    pub value: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn scrub(
    ui: &mut Ui,
    value: f32,
    buffered: Option<f32>,
    width: f32,
    accent: Accent,
    height: f32,
    volume_style: bool,
) -> Scrub {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let mut value = value;
    if let Some(pos) = response.interact_pointer_pos() {
        value = ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
    }
    let painter = ui.painter().clone();
    let active = response.hovered() || response.dragged();
    let track_h = if active { 6.0 } else { 4.0 };
    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), track_h));
    painter.rect_filled(track, track_h * 0.5, theme::white(0.10));

    if let Some(buffered) = buffered {
        let width = track.width() * buffered.clamp(0.0, 1.0);
        if width > 0.0 {
            painter.rect_filled(
                Rect::from_min_size(track.min, Vec2::new(width, track_h)),
                track_h * 0.5,
                theme::white(0.17),
            );
        }
    }

    let filled_w = track.width() * value;
    if filled_w > 0.0 {
        let filled = Rect::from_min_size(track.min, Vec2::new(filled_w, track_h));
        if volume_style {
            painter.rect_filled(filled, track_h * 0.5, theme::white(0.72));
        } else {
            theme::gradient_rounded_rect(
                &painter,
                filled,
                track_h * 0.5,
                accent.deep,
                accent.soft,
                Vec2::new(1.0, 0.0),
            );
        }
    }

    if active {
        let knob = Pos2::new(track.left() + filled_w, track.center().y);
        painter.circle_filled(knob, 5.5, Color32::WHITE);
        if !volume_style {
            painter.circle_stroke(knob, 5.5, Stroke::new(2.0, accent.dim()));
        }
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    Scrub { response, value }
}

// ---------------------------------------------------------------------------
// cover art
// ---------------------------------------------------------------------------

fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let h = h.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= f32::EPSILON {
        let v = (l * 255.0) as u8;
        return Color32::from_rgb(v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        let value = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (value * 255.0) as u8
    };
    Color32::from_rgb(channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0))
}

/// Determinstic placeholder art: the title's first meaningful character over a
/// two-stop gradient seeded by the track key, mirroring `Cover` in the design.
fn cover_seed(track: &Track) -> (String, Color32, Color32) {
    let mut hash: u32 = 2_166_136_261;
    for byte in track.key().bytes() {
        hash = (hash ^ byte as u32).wrapping_mul(16_777_619);
    }
    let h1 = (hash % 360) as f32;
    let h2 = (h1 + 40.0 + ((hash >> 9) % 80) as f32) % 360.0;
    let initial = track
        .title
        .trim_start_matches(|c: char| {
            !c.is_alphanumeric() && !('\u{4e00}'..='\u{9fff}').contains(&c)
        })
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "♪".to_owned());
    (initial, hsl(h1, 0.58, 0.45), hsl(h2, 0.54, 0.21))
}

/// Paints cover art into `rect`. Falls back to a monogram until the real image
/// arrives from the API.
pub fn paint_cover(
    painter: &egui::Painter,
    rect: Rect,
    track: Option<&Track>,
    texture: Option<&TextureHandle>,
) {
    let radius = if rect.width() >= 50.0 { 11.0 } else { 9.0 };
    if let Some(texture) = texture {
        // Keep the rounded frame painted underneath so corners never flash
        // square while the bitmap is being swapped.
        painter.rect_filled(rect, radius, theme::INK_3);
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, theme::white(0.09)),
            egui::StrokeKind::Inside,
        );
        return;
    }

    let (initial, from, to) = match track {
        Some(track) => cover_seed(track),
        None => (
            "♪".to_owned(),
            Color32::from_rgb(0x23, 0x23, 0x2c),
            Color32::from_rgb(0x16, 0x16, 0x1d),
        ),
    };
    theme::gradient_rounded_rect(painter, rect, radius, from, to, Vec2::new(0.7, 0.7));
    // The soft top-left highlight of the design's `.cover::before`.
    theme::bloom(
        painter,
        Pos2::new(
            rect.left() + rect.width() * 0.3,
            rect.top() + rect.height() * 0.2,
        ),
        rect.width() * 0.6,
        Color32::WHITE,
        if track.is_some() { 0.18 } else { 0.05 },
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, theme::white(0.09)),
        egui::StrokeKind::Inside,
    );

    paint_label(
        painter,
        rect.center(),
        Align2::CENTER_CENTER,
        initial,
        theme::lyric_font(rect.width() * 0.44),
        if track.is_some() {
            Color32::from_rgba_unmultiplied(255, 255, 255, 237)
        } else {
            theme::white(0.5)
        },
    );
}

// ---------------------------------------------------------------------------
// indicators
// ---------------------------------------------------------------------------

/// The four animated bars that mark "this is playing".
pub fn equalizer(ui: &Ui, rect: Rect, playing: bool, color: Color32) {
    let painter = ui.painter().clone();
    let time = ui.input(|i| i.time) as f32;
    let bar_w = rect.width() * 0.16;
    let gap = (rect.width() - bar_w * 4.0) / 3.0;
    const BASE: [f32; 4] = [0.6, 1.0, 0.76, 0.44];
    for (index, base) in BASE.iter().enumerate() {
        let factor = if playing {
            let phase = time * 3.6 + index as f32 * 0.85;
            (base + 0.34 * phase.sin()).clamp(0.28, 1.0)
        } else {
            *base
        };
        let height = rect.height() * factor;
        let bar = Rect::from_min_size(
            Pos2::new(
                rect.left() + index as f32 * (bar_w + gap),
                rect.bottom() - height,
            ),
            Vec2::new(bar_w, height),
        );
        painter.rect_filled(bar, bar_w * 0.5, color);
    }
}

pub struct Badge {
    pub text: &'static str,
    pub fg: Color32,
    pub bg: Color32,
}

impl Badge {
    pub fn quality(quality: Option<AudioQuality>) -> Badge {
        match quality {
            Some(AudioQuality::Flac) => Badge {
                text: "FLAC",
                fg: theme::GOLD,
                bg: theme::fade(theme::GOLD, 0.15),
            },
            Some(AudioQuality::K192) => Badge {
                text: "192K",
                fg: theme::BLUE_INK,
                bg: theme::fade(theme::BLUE, 0.14),
            },
            Some(AudioQuality::K132) => Badge {
                text: "132K",
                fg: theme::FG_2,
                bg: theme::white(0.055),
            },
            // YouTube's two AAC grades. Neither is a Bilibili stream id, so they
            // take the neutral tint rather than a quality colour.
            Some(AudioQuality::YtAac128) => Badge {
                text: "AAC 130K",
                fg: theme::FG_2,
                bg: theme::white(0.055),
            },
            Some(AudioQuality::YtAac48) => Badge {
                text: "AAC 50K",
                fg: theme::FG_2,
                bg: theme::white(0.055),
            },
            Some(AudioQuality::K64) => Badge {
                text: "64K",
                fg: theme::WARN,
                bg: theme::fade(theme::WARN, 0.14),
            },
            None => Badge {
                text: "解析中",
                fg: theme::FG_3,
                bg: theme::white(0.055),
            },
        }
    }

    pub fn restricted() -> Badge {
        Badge {
            text: "大会员专享",
            fg: theme::WARN,
            bg: theme::fade(theme::WARN, 0.14),
        }
    }
}

/// Paints a badge right-aligned inside `rect`; returns its width.
pub fn paint_badge(ui: &Ui, rect: Rect, badge: &Badge) -> f32 {
    let font = mono_font(10.0);
    let text_w = measure(ui, badge.text, &font).x;
    let width = text_w + 14.0;
    let pill = Rect::from_min_size(
        Pos2::new(rect.right() - width, rect.center().y - 9.5),
        Vec2::new(width, 19.0),
    );
    ui.painter().rect_filled(pill, 5.0, badge.bg);
    paint_label(
        ui.painter(),
        pill.center(),
        Align2::CENTER_CENTER,
        badge.text,
        font,
        badge.fg,
    );
    width
}

// ---------------------------------------------------------------------------
// states
// ---------------------------------------------------------------------------

/// The centred "nothing here" panel from the design: a disc of art, a title, a
/// description and optional suggestion chips. Returns a clicked suggestion.
pub fn empty_state(
    ui: &mut Ui,
    icon: Icon,
    title: &str,
    desc: &str,
    suggestions: &[&str],
    accent: Accent,
) -> Option<String> {
    let mut clicked = None;
    ui.vertical_centered(|ui| {
        ui.add_space(44.0);
        let (art_rect, _) = ui.allocate_exact_size(Vec2::splat(92.0), Sense::hover());
        let painter = ui.painter().clone();
        theme::bloom(&painter, art_rect.center(), 52.0, Color32::WHITE, 0.06);
        painter.circle_filled(art_rect.center(), 46.0, theme::INK_2);
        painter.circle_stroke(art_rect.center(), 46.0, Stroke::new(1.0, theme::LINE));
        painter.circle_stroke(art_rect.center(), 38.0, Stroke::new(1.0, theme::LINE));
        icon(&painter, art_rect.shrink(33.0), theme::FG_3);

        ui.add_space(16.0);
        ui.label(egui::RichText::new(title).size(15.0).color(theme::FG_2));
        ui.add_space(6.0);
        if !desc.is_empty() {
            centred_paragraph(ui, desc, ui_font(12.5), theme::FG_3, 380.0, 1.75);
        }
        if !suggestions.is_empty() {
            ui.add_space(14.0);
            // A `horizontal` block stretches to the full width, so the chips are
            // centred by hand rather than by the parent layout.
            let gap = 8.0;
            let total: f32 = suggestions
                .iter()
                .map(|s| measure(ui, s, ui_font(12.0)).x + 22.0)
                .sum::<f32>()
                + gap * (suggestions.len().saturating_sub(1)) as f32;
            let indent = ((ui.available_width() - total) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(indent);
                for (index, suggestion) in suggestions.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(gap);
                    }
                    if chip(ui, suggestion, None, None, false, accent).clicked() {
                        clicked = Some((*suggestion).to_owned());
                    }
                }
            });
        }
    });
    clicked
}

/// Shimmering placeholder rows while a list loads.
pub fn skeleton(ui: &mut Ui, rows: usize, row_h: f32) {
    let time = ui.input(|i| i.time) as f32;
    let phase = (time * 1.4).rem_euclid(1.0);
    let alpha = 0.035 + 0.045 * (phase * std::f32::consts::TAU).sin().abs();
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.vertical(|ui| {
            // Measure once: `available_width` shrinks as rows are allocated.
            let width = (ui.available_width() - 14.0).max(40.0);
            for _ in 0..rows {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(width, row_h - 2.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, theme::R_MD, theme::white(alpha));
                ui.add_space(1.0);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The row step of the first wrapped galley, i.e. the leading in pixels.
    ///
    /// ASCII on purpose: the assertion is about the leading, and a CJK sample
    /// would lay out differently on a machine without a Chinese font.
    fn leading_of(text: &str, build: impl FnOnce(&mut Ui)) -> f32 {
        let ctx = egui::Context::default();
        let mut build = Some(build);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(build) = build.take() {
                build(ui);
            }
        });
        // epaint's font atlas arrives as a texture delta that a headless test
        // has nothing to upload; dropping it unhandled panics.
        output.textures_delta.clear();
        for clipped in output.shapes {
            if let egui::Shape::Text(text_shape) = clipped.shape {
                let rows = &text_shape.galley.rows;
                assert!(
                    rows.len() >= 2,
                    "{text:?} should have wrapped into two rows"
                );
                return rows[1].pos.y - rows[0].pos.y;
            }
        }
        panic!("{text:?} should have been painted as text");
    }

    #[test]
    fn hints_keep_the_designs_leading() {
        let text = "The quick brown fox jumps over the lazy dog, and then does it again.";
        let step = leading_of(text, |ui| {
            ui.set_max_width(220.0);
            hint(ui, text, 11.0, theme::FG_3);
        });
        let expected = 11.0 * HINT_LEADING;
        assert!(
            (step - expected).abs() < 0.75,
            "hint leading should be {expected}, got {step}"
        );
    }

    #[test]
    fn paragraphs_keep_the_designs_leading() {
        let text = "The quick brown fox jumps over the lazy dog, and then does it again.";
        let step = leading_of(text, |ui| {
            centred_paragraph(ui, text, ui_font(12.5), theme::FG_3, 240.0, 1.75);
        });
        let expected = 12.5 * 1.75;
        assert!(
            (step - expected).abs() < 0.75,
            "paragraph leading should be {expected}, got {step}"
        );
    }
}

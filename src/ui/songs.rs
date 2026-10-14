//! The song row, and the list scaffolding around it.
//!
//! One row component serves 我的歌 / 最近播放 / 已下载 / 搜索结果, because in all four
//! places the thing in the row is the same thing: a song. That is the whole point
//! of the redesign — the old row was a *platform entry*, so it carried the
//! uploader, the view count and the `bvid`, and there was no way to show a song
//! that two platforms both hold.
//!
//! What this row does **not** carry is the platform. Which services hold the
//! recording, at what bitrate, and whether the preferred copy is still alive is
//! real information, but it belongs behind a 来源 button rather than in the line
//! you scan while deciding what to listen to. What it does carry is *availability*,
//! because "can I play this at all" has to be answerable before you press play:
//! 已下载 / 在线 / 已回退 / 仅前半 / 全部失效.

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use super::icons::Icon;

use crate::api::models::Track;

use super::theme::{self as t, Led};
use super::widgets;

/// Widths of the fixed columns, right to left. The title takes what is left.
const COL_INDEX: f32 = 36.0;
const COL_COVER: f32 = 40.0;
const COL_TIME: f32 = 56.0;
const COL_STATE: f32 = 96.0;
// Wide enough for the widest thing this column holds, `10 月 13 日` (12 月 30 日
// is wider still), plus the gap the state label needs. Hand-derived columns are
// only as good as this number: too small and the two overlap, which is how the
// date lost its leading digits the first time round.
const COL_META: f32 = 116.0;
const COL_ACTION: f32 = 78.0;
/// Inset from the pane edge to the first column.
const SIDE: f32 = 16.0;

/// What the state column says, and which lamp it lights.
///
/// Precedence is about what a listener needs to know *before* pressing play, so
/// the states that mean "this will not work as expected" outrank the pleasant
/// ones: a song that fell back to another platform is worth flagging even if its
/// copy is also downloaded.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SongState {
    /// Every edition is gone. The row is drawn dimmed and does not respond.
    Gone,
    /// The preferred edition is gone and another platform's copy is playing.
    Fallback,
    /// The file is only partly on disk.
    Partial,
    /// Fully downloaded.
    Downloaded,
    /// Streams on play.
    Online,
}

impl SongState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SongState::Gone => "全部失效",
            SongState::Fallback => "已回退",
            SongState::Partial => "仅前半",
            SongState::Downloaded => "已下载",
            SongState::Online => "在线",
        }
    }

    pub(crate) fn led(self) -> Led {
        match self {
            SongState::Gone => Led::Bad,
            SongState::Fallback | SongState::Partial => Led::Warn,
            SongState::Downloaded => Led::Ok,
            SongState::Online => Led::Idle,
        }
    }

    /// A dead row is still listed — deleting a song because a platform removed a
    /// video would be the app quietly losing your music — but it cannot be played.
    pub(crate) fn playable(self) -> bool {
        self != SongState::Gone
    }
}

/// Everything the row draws, lifted out of whatever it came from.
///
/// A borrowed view rather than `&LibraryEntry` on purpose: the row should not
/// know whether it is being fed the library, a search result or the cache, and
/// this keeps it compiling (and testable) without any of them.
pub(crate) struct SongRow<'a> {
    pub title: &'a str,
    /// The singer when a source told us one, otherwise the uploader or channel —
    /// with `artist_known` saying which. Neither platform's search gives an
    /// artist, so this is usually the uploader, and painting a re-upload account
    /// in the artist's place without distinguishing it is a small lie.
    pub artist: &'a str,
    pub artist_known: bool,
    pub duration: u64,
    pub initial: String,
    pub hue: f32,
    pub state: SongState,
    /// Trailing column: 加入时间 in the library, 播放时间 in 最近播放，大小 in 已下载.
    pub meta: String,
    pub playing: bool,
}

impl<'a> SongRow<'a> {
    pub(crate) fn from_track(
        track: &'a Track,
        state: SongState,
        meta: String,
        playing: bool,
    ) -> Self {
        let initial = track
            .title
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let artist = track.artist.as_deref().filter(|a| !a.is_empty());
        Self {
            title: &track.title,
            artist: artist.unwrap_or(&track.author),
            artist_known: artist.is_some(),
            duration: track.duration,
            hue: cover_hue(track),
            initial,
            state,
            meta,
            playing,
        }
    }
}

/// A stable, low-saturation cover tint for a recording.
///
/// Keyed off the track so the same song keeps the same tint on every page, and
/// deliberately desaturated: these are placeholder monograms, and a saturated
/// thumbnail would compete with the lamps that carry the actual state.
pub(crate) fn cover_hue(track: &Track) -> f32 {
    // FNV-1a over the platform-qualified key: cheap, stable, and independent of
    // hash-map iteration order.
    let mut hash: u32 = 0x811c_9dc5;
    for byte in track.key().as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash % 360) as f32
}

/// Paints the cover monogram. Two stops of the same hue, dark enough that the
/// equaliser overlay still reads on top of it.
pub(crate) fn paint_cover(
    ui: &Ui,
    painter: &egui::Painter,
    rect: Rect,
    row: &SongRow,
    playing: bool,
    accent: t::Accent,
) {
    let from = hsl(row.hue, 0.15, 0.25);
    let to = hsl(row.hue, 0.20, 0.12);
    t::gradient_rounded_rect(painter, rect, t::R_SM, from, to, Vec2::new(0.6, 0.9));
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, t::LINE_2),
        egui::StrokeKind::Inside,
    );
    if playing {
        painter.rect_filled(
            rect,
            t::R_SM,
            Color32::from_rgba_unmultiplied(8, 8, 10, 158),
        );
        widgets::equalizer(ui, rect.shrink(6.0), true, accent.accent);
    } else {
        widgets::label_at(
            ui,
            rect.center(),
            Align2::CENTER_CENTER,
            &row.initial,
            t::ui_font(13.0),
            Color32::from_rgba_unmultiplied(255, 255, 255, 184),
        );
    }
}

/// What a row can do. Consumed by the page that owns the list.
///
/// One enum for both the trailing button and the `⋯` menu's choices, because from
/// the page's side they are the same thing: something the user asked for on this
/// row, on its way to being handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SongAction {
    /// Play this song now.
    Play,
    /// Put it in the library — the one action a search result that is not yours
    /// has to offer.
    AddToLibrary,
    /// Remove from the local library.
    Remove,
    /// Take the song out of the playlist being shown, leaving the library alone.
    RemoveFromPlaylist,
    /// Delete the local copy.
    DeleteDownload,
    /// The `⋯` menu.
    CopyLink,
    OpenInBrowser,
    Enqueue,
    EnqueueNext,
    Cache,
    Export,
}

impl SongAction {
    /// The trailing button's text. `None` for the actions that only appear in
    /// the `⋯` menu.
    fn label(self) -> Option<&'static str> {
        match self {
            SongAction::AddToLibrary => Some("加入"),
            SongAction::Remove | SongAction::RemoveFromPlaylist => Some("移出"),
            SongAction::DeleteDownload => Some("删除"),
            _ => None,
        }
    }

    /// A leading glyph for the trailing button.
    fn icon(self) -> Option<Icon> {
        match self {
            SongAction::AddToLibrary => Some(super::icons::plus),
            _ => None,
        }
    }
}

/// What a row's trailing cell is.
///
/// The design needs three states, and the third is the one worth naming: a search
/// result that is already in the library has nothing to offer, and a button that
/// does nothing is worse than a dim tick that says why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowAction {
    /// A button carrying one action.
    Button(SongAction),
    /// Nothing to press: the song is already yours.
    Settled,
    /// The `⋯` menu.
    Menu,
}

/// `artist · album`, for the line under a title in either column.
///
/// A track carries no year, so the design's third part is simply absent rather
/// than invented, and an uploader standing in for the singer is still shown —
/// it is the only name there is.
pub(crate) fn subtitle(track: &Track) -> String {
    let artist = track
        .artist
        .as_deref()
        .filter(|artist| !artist.is_empty())
        .unwrap_or(&track.author);
    let mut parts = vec![artist.to_owned()];
    if let Some(album) = track.album.as_deref().filter(|album| !album.is_empty()) {
        parts.push(album.to_owned());
    }
    parts.join(" · ")
}

/// Draws one song. Returns whatever was clicked.
///
/// `action` is the row's trailing cell: a button, a dim tick, or the `⋯` menu.
/// Passing the action rather than its label is what makes the button return the
/// thing it says on it.
pub(crate) fn song_row(
    ui: &mut Ui,
    accent: t::Accent,
    row: &SongRow,
    index: usize,
    action: RowAction,
) -> Option<SongAction> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t::ROW_H),
        if row.state.playable() {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let painter = ui.painter().clone();
    let hovered = response.hovered() && row.state.playable();
    let dim = row.state == SongState::Gone;

    if row.playing {
        t::gradient_rounded_rect(
            &painter,
            rect,
            t::R_SM,
            t::fade(accent.accent, 0.10),
            Color32::TRANSPARENT,
            Vec2::new(1.0, 0.0),
        );
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
            0.0,
            accent.accent,
        );
    } else if hovered {
        painter.rect_filled(rect, t::R_SM, t::INK_2);
    }
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, t::LINE),
    );

    let fade = |color: Color32| {
        if dim {
            t::fade(color, 0.42)
        } else {
            color
        }
    };

    // ---- columns, laid out from the right so the title absorbs the slack -----
    let action_rect = Rect::from_min_size(
        Pos2::new(rect.right() - SIDE - COL_ACTION, rect.top()),
        Vec2::new(COL_ACTION, rect.height()),
    );
    // Every column is separated by the design's 8px gutter, so the header and the
    // row derive their edges the same way and cannot drift apart.
    let meta_right = action_rect.left() - 8.0;
    let state_left = meta_right - COL_META - 8.0;
    let time_right = state_left - COL_STATE - 8.0;
    let title_right = time_right - COL_TIME - 8.0;

    // index
    let index_rect = Rect::from_min_size(
        Pos2::new(rect.left() + SIDE, rect.top()),
        Vec2::new(COL_INDEX, rect.height()),
    );
    let index_text = format!("{:02}", index + 1);
    widgets::label_at(
        ui,
        index_rect.center(),
        Align2::CENTER_CENTER,
        &index_text,
        t::mono_font(11.0),
        fade(if row.playing { accent.accent } else { t::FG_4 }),
    );

    // cover
    let cover_size = 30.0;
    let cover_rect = Rect::from_center_size(
        Pos2::new(index_rect.right() + COL_COVER / 2.0, rect.center().y),
        Vec2::splat(cover_size),
    );
    paint_cover(ui, &painter, cover_rect, row, row.playing, accent);

    // title / artist
    let text_rect = Rect::from_min_max(
        Pos2::new(cover_rect.right() + 14.0, rect.top() + 8.0),
        Pos2::new(title_right, rect.bottom() - 8.0),
    );
    widgets::clipped_line(
        ui,
        Rect::from_min_size(text_rect.min, Vec2::new(text_rect.width(), 18.0)),
        row.title,
        t::ui_font(13.5),
        fade(if row.playing { accent.ink } else { t::FG }),
        false,
    );
    // An uploader's name where the singer should be is dimmed rather than hidden:
    // it is the only name there is, and it still helps you recognise the track.
    widgets::clipped_line(
        ui,
        Rect::from_min_size(
            Pos2::new(text_rect.left(), text_rect.top() + 21.0),
            Vec2::new(text_rect.width(), 16.0),
        ),
        row.artist,
        t::ui_font(11.5),
        fade(if row.artist_known { t::FG_3 } else { t::FG_4 }),
        false,
    );

    // duration
    let time_rect = Rect::from_min_size(
        Pos2::new(time_right - COL_TIME, rect.top()),
        Vec2::new(COL_TIME, rect.height()),
    );
    widgets::label_at(
        ui,
        Pos2::new(time_rect.right(), time_rect.center().y),
        Align2::RIGHT_CENTER,
        &crate::util::format_duration(row.duration),
        t::mono_font(11.5),
        fade(t::FG_3),
    );

    // state
    let state_rect = Rect::from_min_size(
        Pos2::new(state_left, rect.top()),
        Vec2::new(COL_STATE, rect.height()),
    );
    let lamp = Pos2::new(state_rect.left() + 4.0, state_rect.center().y);
    t::led(&painter, lamp, row.state.led(), accent);
    widgets::label_at(
        ui,
        Pos2::new(lamp.x + 12.0, lamp.y),
        Align2::LEFT_CENTER,
        row.state.label(),
        t::mono_font(10.0),
        fade(match row.state {
            SongState::Gone => t::ERR,
            _ => t::FG_3,
        }),
    );

    // meta (加入 / 播放于 / 大小)
    widgets::label_at(
        ui,
        Pos2::new(meta_right, rect.center().y),
        Align2::RIGHT_CENTER,
        &row.meta,
        t::mono_font(10.0),
        fade(t::FG_4),
    );

    // ---- trailing control ---------------------------------------------------
    let mut acted = None;
    match action {
        RowAction::Button(action) => {
            let label = action.label().unwrap_or("");
            let button =
                Rect::from_center_size(action_rect.center(), Vec2::new(COL_ACTION - 10.0, 22.0));
            let response = ui.interact(
                button,
                ui.make_persistent_id(("song-add", row.title, index)),
                Sense::click(),
            );
            let (fill, stroke, fg) = if response.hovered() {
                (t::INK_3, t::LINE_3, t::FG)
            } else {
                (Color32::TRANSPARENT, t::LINE_2, t::FG_2)
            };
            painter.rect_filled(button, t::R_SM, fill);
            painter.rect_stroke(
                button,
                t::R_SM,
                Stroke::new(1.0, stroke),
                egui::StrokeKind::Inside,
            );
            let mut x = button.center().x;
            if let Some(icon) = action.icon() {
                let width = widgets::measure(ui, label, t::ui_font(11.0)).x + 15.0;
                x -= width / 2.0;
                icon(
                    &painter,
                    Rect::from_center_size(
                        Pos2::new(x + 5.0, button.center().y),
                        Vec2::splat(11.0),
                    ),
                    fg,
                );
                widgets::paint_label(
                    &painter,
                    Pos2::new(x + 14.0, button.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    t::ui_font(11.0),
                    fg,
                );
            } else {
                widgets::paint_label(
                    &painter,
                    button.center(),
                    Align2::CENTER_CENTER,
                    label,
                    t::ui_font(11.0),
                    fg,
                );
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                acted = Some(action);
            }
        }
        RowAction::Settled => {
            // A dim tick, not a button: there is nothing to do to a song that is
            // already in the library, and the row says so instead of offering a
            // control that would do nothing.
            super::icons::check(
                &painter,
                Rect::from_center_size(action_rect.center(), Vec2::splat(14.0)),
                t::fade(t::FG_3, 0.45),
            );
        }
        RowAction::Menu if row.state.playable() => {
            let more = Rect::from_center_size(action_rect.center(), Vec2::splat(18.0));
            let response = ui.interact(
                more.expand(4.0),
                ui.make_persistent_id(("song-more", row.title, index)),
                Sense::click(),
            );
            // Dim rather than hidden: a row action that only exists on hover
            // cannot be reviewed in a screenshot.
            let alpha = if response.hovered() { 1.0 } else { 0.34 };
            super::icons::more(
                &painter,
                more,
                t::fade(if response.hovered() { t::FG } else { t::FG_3 }, alpha),
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            egui::Popup::menu(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .align(egui::emath::RectAlign::BOTTOM_END)
                .gap(4.0)
                .frame(
                    egui::Frame::NONE
                        .fill(t::INK_3)
                        .corner_radius(t::R_LG)
                        .inner_margin(egui::Margin::same(5))
                        .stroke(Stroke::new(1.0, t::LINE_2)),
                )
                .show(|ui| {
                    ui.set_min_width(186.0);
                    let items: [(&str, Icon, SongAction); 6] = [
                        ("复制视频链接", super::icons::copy, SongAction::CopyLink),
                        (
                            "在浏览器打开",
                            super::icons::external,
                            SongAction::OpenInBrowser,
                        ),
                        ("加入播放列表", super::icons::queue, SongAction::Enqueue),
                        ("下一首播放", super::icons::next, SongAction::EnqueueNext),
                        ("缓存到本地", super::icons::download, SongAction::Cache),
                        ("导出为 m4a", super::icons::folder, SongAction::Export),
                    ];
                    for (label, icon, action) in items {
                        if menu_item(ui, label, icon, false) {
                            acted = Some(action);
                        }
                    }
                });
        }
        RowAction::Menu => {}
    }

    if acted.is_some() {
        return acted;
    }
    if response.clicked() && row.state.playable() {
        return Some(SongAction::Play);
    }
    None
}

/// One line of a row's `⋯` menu.
fn menu_item(ui: &mut Ui, label: &str, icon: Icon, danger: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered();
    let fg = if danger {
        if hovered {
            t::ERR
        } else {
            t::FG_2
        }
    } else if hovered {
        t::FG
    } else {
        t::FG_2
    };
    if hovered {
        painter.rect_filled(rect, t::R_SM, t::white(0.07));
    }
    icon(
        &painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 16.0, rect.center().y),
            Vec2::splat(13.0),
        ),
        if danger { fg } else { t::FG_3 },
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        t::ui_font(12.0),
        fg,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// The column header above a song list. Same widths as the row, so the two can
/// never drift apart.
pub(crate) fn song_columns(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, t::INK_1);
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, t::LINE),
    );

    // Same arithmetic as `song_row`, so the header can never drift from the row.
    let action_left = rect.right() - SIDE - COL_ACTION;
    let meta_right = action_left - 8.0;
    let state_left = meta_right - COL_META - 8.0;
    let time_right = state_left - COL_STATE - 8.0;
    let title_left = rect.left() + SIDE + COL_INDEX + COL_COVER + 14.0;

    let labels: [(f32, Align2, &str); 4] = [
        (title_left, Align2::LEFT_CENTER, "歌曲"),
        (time_right, Align2::RIGHT_CENTER, "时长"),
        (state_left, Align2::LEFT_CENTER, "状态"),
        (meta_right, Align2::RIGHT_CENTER, "加入"),
    ];
    for (x, anchor, text) in labels {
        widgets::engraved_at(
            ui,
            Pos2::new(x, rect.center().y),
            anchor,
            text,
            9.0,
            t::FG_4,
        );
    }
}

/// A day / playlist divider inside a list.
pub(crate) fn group_header(ui: &mut Ui, title: &str, detail: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, t::INK_1);
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, t::LINE),
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + SIDE, rect.center().y + 2.0),
        Align2::LEFT_CENTER,
        title,
        t::ui_font(12.5),
        t::FG_2,
    );
    if !detail.is_empty() {
        let width = widgets::measure(ui, title, t::ui_font(12.5)).x;
        widgets::label_at(
            ui,
            Pos2::new(rect.left() + SIDE + width + 12.0, rect.center().y + 2.0),
            Align2::LEFT_CENTER,
            detail,
            t::mono_font(9.5),
            t::FG_4,
        );
    }
}

/// `hsl` in the one form the covers need — low saturation, dark.
fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color32 {
    let h = hue.rem_euclid(360.0) / 360.0;
    let s = saturation.clamp(0.0, 1.0);
    let l = lightness.clamp(0.0, 1.0);
    let channel = |n: f32| {
        let k = (n + h * 12.0) % 12.0;
        let a = s * l.min(1.0 - l);
        let value = l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0);
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color32::from_rgb(channel(0.0), channel(8.0), channel(4.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{Source, Track};
    use egui::{Event, Modifiers, PointerButton, RawInput};

    fn raw(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 400.0))),
            events,
            ..Default::default()
        }
    }

    fn texts(output: &egui::FullOutput) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for shape in &output.shapes {
            walk(&shape.shape, &mut out);
        }
        out
    }

    /// The rect a painted label was drawn in, so a test can click it.
    fn label_rect(output: &egui::FullOutput, label: &str) -> Option<Rect> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(text) => {
                    out.push((text.galley.text().to_owned(), text.visual_bounding_rect()));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut all = Vec::new();
        for shape in &output.shapes {
            walk(&shape.shape, &mut all);
        }
        all.into_iter()
            .find(|(text, _)| text == label)
            .map(|(_, rect)| rect)
    }

    /// One row on its own, plus whatever it asked for.
    fn render(
        ctx: &egui::Context,
        row: &SongRow,
        action: RowAction,
        events: Vec<Event>,
    ) -> (Option<SongAction>, egui::FullOutput) {
        let mut acted = None;
        let mut output = ctx.run_ui(raw(events), |ui| {
            acted = song_row(ui, t::ACCENTS[0], row, 0, action);
        });
        output.textures_delta.clear();
        (acted, output)
    }

    fn click(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &crate::ui::theme::Theme::default());
        ctx
    }

    /// Where the `⋯` and the trailing button live, derived from the row's own
    /// constants rather than hard-coded: a layout change should move the target.
    fn action_centre() -> Pos2 {
        Pos2::new(
            1280.0 - SIDE - COL_ACTION / 2.0,
            crate::ui::theme::ROW_H / 2.0,
        )
    }

    #[test]
    fn a_playing_row_marks_itself_on_the_cover_not_in_the_number_column() {
        // The design keeps the number and swaps the artwork for an equaliser, so
        // "which one is playing" is said by the cover rather than by the index.
        let ctx = context();
        let song = track("晴天", "杰威尔音乐");

        let idle = SongRow::from_track(&song, SongState::Online, String::new(), false);
        let (_, output) = render(&ctx, &idle, RowAction::Menu, vec![]);
        let painted = texts(&output);
        assert!(
            painted.iter().any(|text| text == "01"),
            "the number is always there"
        );
        assert!(
            painted.iter().any(|text| text == "晴"),
            "and so is the monogram"
        );

        let playing = SongRow::from_track(&song, SongState::Online, String::new(), true);
        let (_, output) = render(&ctx, &playing, RowAction::Menu, vec![]);
        let painted = texts(&output);
        assert!(painted.iter().any(|text| text == "01"), "the number stays");
        assert!(
            !painted.iter().any(|text| text == "晴"),
            "the monogram gives way to the equaliser: {painted:?}"
        );
    }

    #[test]
    fn clicking_a_row_asks_to_play_it() {
        let ctx = context();
        let song = track("晴天", "杰威尔音乐");
        let row = SongRow::from_track(&song, SongState::Online, String::new(), false);
        let (_, laid_out) = render(&ctx, &row, RowAction::Menu, vec![]);
        let title = label_rect(&laid_out, "晴天").expect("the title is painted");

        let (acted, _) = render(&ctx, &row, RowAction::Menu, click(title.center()));
        assert_eq!(acted, Some(SongAction::Play));
    }

    #[test]
    fn a_button_row_returns_the_action_it_shows() {
        let ctx = context();
        let song = track("晴天", "杰威尔音乐");
        let row = SongRow::from_track(&song, SongState::Online, String::new(), false);
        let action = RowAction::Button(SongAction::AddToLibrary);
        let (_, laid_out) = render(&ctx, &row, action, vec![]);
        let button = label_rect(&laid_out, "加入").expect("the button is painted");

        let (acted, _) = render(&ctx, &row, action, click(button.center()));
        assert_eq!(acted, Some(SongAction::AddToLibrary));
    }

    #[test]
    fn a_settled_row_offers_nothing_to_press() {
        let ctx = context();
        let song = track("晴天", "杰威尔音乐");
        let row = SongRow::from_track(&song, SongState::Online, "已在库".to_owned(), false);
        let (_, laid_out) = render(&ctx, &row, RowAction::Settled, vec![]);
        let painted = texts(&laid_out);
        assert!(painted.iter().any(|text| text == "已在库"));
        assert!(
            !painted.iter().any(|text| text == "加入"),
            "a song already in the library has nothing to offer: {painted:?}"
        );

        // The cell is empty, not a dead zone: the row itself is still what you
        // press to play the song, exactly as the design has it.
        let (acted, _) = render(&ctx, &row, RowAction::Settled, click(action_centre()));
        assert_eq!(acted, Some(SongAction::Play));
    }

    #[test]
    fn the_overflow_menu_lists_what_a_row_can_do() {
        let ctx = context();
        let song = track("晴天", "杰威尔音乐");
        let row = SongRow::from_track(&song, SongState::Online, String::new(), false);
        // The `⋯` opens a popup, and a popup paints nothing on the pass it is
        // first laid out — hence the warm-up pass and the click's own pass.
        render(&ctx, &row, RowAction::Menu, vec![]);
        let (acted, _) = render(&ctx, &row, RowAction::Menu, click(action_centre()));
        let (_, laid_out) = render(&ctx, &row, RowAction::Menu, vec![]);

        let painted = texts(&laid_out);
        for label in [
            "复制视频链接",
            "在浏览器打开",
            "加入播放列表",
            "下一首播放",
            "缓存到本地",
            "导出为 m4a",
        ] {
            assert!(
                painted.iter().any(|text| text == label),
                "{label} should be in the menu: {painted:?}"
            );
        }
        assert_eq!(acted, None, "opening the menu is not itself an action");
    }

    fn track(title: &str, author: &str) -> Track {
        Track {
            // One id per title: both `Track::key` and the cover tint are derived
            // from it, so a shared id would make two songs one recording.
            bvid: format!("BV{title}"),
            source: Source::Bilibili,
            aid: 1,
            cid: 2,
            title: title.into(),
            author: author.into(),
            duration: 269,
            cover: None,
            album: None,
            artist: None,
        }
    }

    #[test]
    fn a_dead_song_is_still_listed_but_cannot_play() {
        // Deleting a song because a platform removed a video would be the app
        // quietly losing your music, so the row stays and refuses to play.
        assert!(!SongState::Gone.playable());
        assert!(SongState::Fallback.playable());
        assert_eq!(SongState::Gone.label(), "全部失效");
    }

    #[test]
    fn the_state_lamp_distinguishes_fell_back_from_downloaded() {
        assert_eq!(SongState::Fallback.led(), Led::Warn);
        assert_eq!(SongState::Downloaded.led(), Led::Ok);
        assert_ne!(SongState::Fallback.led(), SongState::Downloaded.led());
    }

    #[test]
    fn the_cover_tint_is_stable_for_one_recording() {
        let a = track("晴天", "杰威尔音乐");
        let b = track("晴天", "杰威尔音乐");
        assert!((cover_hue(&a) - cover_hue(&b)).abs() < f32::EPSILON);
        // …and differs between recordings, or every cover would look the same.
        assert!((cover_hue(&a) - cover_hue(&track("七里香", "杰威尔音乐"))).abs() > 0.5);
    }

    #[test]
    fn a_known_artist_wins_over_the_uploader() {
        // `author` is the uploader or channel name, not the singer. Showing a
        // re-upload account as the artist is worse than showing nothing.
        let mut t = track("晴天", "华语音乐搬运工");
        t.artist = Some("周杰伦".into());
        let row = SongRow::from_track(&t, SongState::Online, String::new(), false);
        assert_eq!(row.artist, "周杰伦");
    }
}

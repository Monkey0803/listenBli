//! The library rail.
//!
//! The old shell organised itself around one platform at a time: a source switch,
//! then search / favourites / history for whichever platform was selected. A
//! library has no platform — 我的歌 is every source merged — so the rail lists
//! *places in your own music* instead, and the platforms only appear as a single
//! row near the bottom.
//!
//! Everything here is hand-painted from `rect`s rather than laid out with
//! `ui.horizontal`, because the active item paints a full-bleed plate and a 2px
//! signal bar on the panel's own left edge, which a layout would inset.

use egui::{Align2, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use super::theme::{self as t, Led};
use super::widgets;
use crate::api::models::Source;
use crate::app::{App, Page};

/// The platforms the library can be assembled from.
///
/// A literal for now. When the source registry lands (read/write capability
/// declarations, a source you can add) this becomes its list — which is why the
/// rail counts collections rather than naming them.
pub(crate) const PLATFORMS: [Source; 2] = Source::ALL;

/// Item rows are 30px, matching the design's nav.
const ITEM_H: f32 = 30.0;
/// Left inset for the icon and the label.
const PAD: f32 = 14.0;

impl App {
    pub(crate) fn ui_library_nav(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let rect = self.nav_rect(ui);
        let painter = ui.painter().clone();

        // The rail is ruled off from the page beside it. The status strip at the
        // bottom of the window is ruled the other way, so the seam reads as one
        // plate boundary and not two.
        painter.vline(
            rect.right() - 0.5,
            rect.y_range(),
            Stroke::new(1.0, t::LINE_2),
        );

        let mut y = rect.top() + 12.0;

        // ---- head: what this rail is, and how many collections feed it --------
        let label_pos = Pos2::new(rect.left() + PAD, y + 7.0);
        widgets::engraved_at(
            ui,
            label_pos,
            Align2::LEFT_CENTER,
            "MY LIBRARY",
            9.5,
            t::FG_4,
        );
        let count = format!("{} 处收藏", self.collection_places());
        widgets::engraved_at(
            ui,
            Pos2::new(rect.right() - PAD, label_pos.y),
            Align2::RIGHT_CENTER,
            &count,
            9.5,
            t::FG_4,
        );
        y += 22.0;

        // ---- the places in your music ---------------------------------------
        for (index, page) in Page::KEYS.iter().enumerate() {
            let count = self.nav_count(*page);
            if self.nav_item(ui, &mut y, *page, count) {
                self.set_page(*page);
            }
            // 搜索 sits apart: everything above it is a view of music you already
            // have, and everything below it is a tool.
            if index == 3 {
                y += 8.0;
                painter.hline(rect.x_range().shrink(PAD), y, Stroke::new(1.0, t::LINE));
                y += 9.0;
            }
        }

        y += 8.0;
        painter.hline(rect.x_range().shrink(PAD), y, Stroke::new(1.0, t::LINE));
        y += 9.0;

        if self.nav_item(ui, &mut y, Page::Sources, Some(PLATFORMS.len())) {
            self.set_page(Page::Sources);
        }

        // ---- footer: the one message line, sync state, and 接入来源 -----------
        let foot_top = rect.bottom() - 100.0;
        painter.hline(rect.x_range(), foot_top, Stroke::new(1.0, t::LINE_2));

        // The console has no status bar, so a transient message lives here, right
        // above the sync line. Errors are the point: they have to stay reachable,
        // and a rail that quietly swallowed a failed resolve would be worse than
        // the strip it replaced.
        let mut cursor = foot_top + 12.0;
        // An audio device that would not open is the one message with nowhere else
        // to go now that the status strip is gone, and it is not a passing note:
        // it stays on the line until the app is restarted.
        let engine_error = self.engine_error.clone();
        if let Some(err) = engine_error {
            let galley =
                widgets::elided(ui, &err, t::ui_font(10.5), t::ERR, rect.width() - PAD * 2.0);
            ui.painter()
                .galley(Pos2::new(rect.left() + PAD, cursor), galley, t::ERR);
            cursor += 20.0;
        } else if let Some(status) = self.status.clone() {
            // The status strip that used to own this expiry is gone, so the rail
            // owns it now: a passing note fades, an explanation of why playback
            // stopped waits to be replaced.
            let left = crate::app::STATUS_TTL.saturating_sub(status.at.elapsed());
            if status.sticky || !left.is_zero() {
                let color = if status.is_error { t::ERR } else { t::FG_3 };
                let galley = widgets::elided(
                    ui,
                    &status.text,
                    t::ui_font(10.5),
                    color,
                    rect.width() - PAD * 2.0,
                );
                ui.painter()
                    .galley(Pos2::new(rect.left() + PAD, cursor), galley, color);
                cursor += 20.0;
                if !status.sticky {
                    ui.ctx().request_repaint_after(left);
                }
            }
        }

        let led_center = Pos2::new(rect.left() + PAD + 3.0, cursor + 6.0);
        let (state, line) = self.sync_line();
        t::led(&painter, led_center, state, accent);
        widgets::label_at(
            ui,
            Pos2::new(led_center.x + 12.0, led_center.y),
            Align2::LEFT_CENTER,
            &line,
            t::mono_font(10.0),
            t::FG_3,
        );

        self.nav_connect(ui, rect, accent);
    }

    /// The rail occupies the whole left panel; taking the rect rather than
    /// letting the layout hand it over keeps the items full-bleed.
    fn nav_rect(&self, ui: &Ui) -> Rect {
        ui.max_rect()
    }

    /// How many things a place holds, when that number is known and meaningful.
    ///
    /// `None` renders nothing rather than a zero: an empty count and "no count
    /// available" are different statements, and the rail should not make the
    /// second one look like the first.
    fn nav_count(&self, page: Page) -> Option<usize> {
        match page {
            Page::Library => Some(self.library.entries.len()),
            Page::Playlists => Some(self.library.playlists.len()),
            Page::Recent => Some(self.library.recent.len()),
            Page::Offline => Some(
                // What the page lists, not what is complete: the badge counts the
                // page's items, and a half-downloaded song is one of them.
                self.cache_index
                    .values()
                    .filter(|info| info.state != crate::app::Cached::None)
                    .count(),
            ),
            // 搜索 is a tool; the key hint is more use than a number.
            Page::Search => None,
            Page::Sources => Some(PLATFORMS.len()),
        }
    }

    /// The sync lamp and its line. Until a real sync exists this says so, rather
    /// than inventing a timestamp — the design's promise is that the library
    /// knows where it came from, and a fake one would break exactly that.
    fn sync_line(&self) -> (Led, String) {
        match self.sync_label() {
            Some(at) => (Led::Ok, format!("上次同步 {at}")),
            None => (Led::Idle, "尚未同步".to_owned()),
        }
    }

    /// Draws one nav item and reports whether it was clicked.
    ///
    /// Hand-painted because the selected state is a full-width plate with a 2px
    /// signal bar at the panel's left edge — a bar the layout's padding would
    /// push inward, and which is the only thing telling you where you are.
    fn nav_item(&mut self, ui: &mut Ui, y: &mut f32, page: Page, count: Option<usize>) -> bool {
        let accent = self.theme.accent;
        let rect = self.nav_rect(ui);
        let item = Rect::from_min_size(Pos2::new(rect.left(), *y), Vec2::new(rect.width(), ITEM_H));
        *y += ITEM_H;

        let response = ui.interact(
            item,
            ui.make_persistent_id(("nav", page.tag())),
            Sense::click(),
        );
        let selected = self.page == page;
        let painter = ui.painter().clone();

        if selected {
            painter.rect_filled(item, 0.0, t::INK_4);
            painter.rect_filled(
                Rect::from_min_size(item.min, Vec2::new(2.0, item.height())),
                0.0,
                accent.accent,
            );
        } else if response.hovered() {
            painter.rect_filled(item, 0.0, t::INK_3);
        }

        let icon_color = match (selected, response.hovered()) {
            (true, _) => accent.accent,
            (false, true) => t::FG_2,
            _ => t::FG_4,
        };
        let icon_rect = Rect::from_center_size(
            Pos2::new(item.left() + PAD + 6.5, item.center().y),
            Vec2::splat(15.0),
        );
        page.icon()(ui.painter(), icon_rect, icon_color);

        let (label_color, label_weight) = match (selected, response.hovered()) {
            (true, _) => (t::FG, true),
            (false, true) => (t::FG, false),
            _ => (t::FG_2, false),
        };
        let label = page.label();
        let label_font = t::ui_font(13.0);
        let label_pos = Pos2::new(item.left() + PAD + 22.0, item.center().y);
        if label_weight {
            widgets::label_at_bold(
                ui,
                label_pos,
                Align2::LEFT_CENTER,
                label,
                label_font,
                label_color,
            );
        } else {
            widgets::label_at(
                ui,
                label_pos,
                Align2::LEFT_CENTER,
                label,
                label_font,
                label_color,
            );
        }

        match count {
            Some(n) => widgets::label_at(
                ui,
                Pos2::new(item.right() - PAD, item.center().y),
                Align2::RIGHT_CENTER,
                &n.to_string(),
                t::mono_font(10.0),
                if selected { accent.accent } else { t::FG_4 },
            ),
            None if page == Page::Search => widgets::label_at(
                ui,
                Pos2::new(item.right() - PAD, item.center().y),
                Align2::RIGHT_CENTER,
                "⌘K",
                t::mono_font(10.0),
                t::FG_4,
            ),
            None => {}
        }

        response.clicked()
    }

    /// 接入来源: a dashed slot at the foot of the rail. It is the one place the
    /// app admits the platform list is not fixed.
    fn nav_connect(&mut self, ui: &mut Ui, rail: Rect, accent: t::Accent) {
        let rect = Rect::from_min_size(
            Pos2::new(rail.left() + PAD, rail.bottom() - 56.0),
            Vec2::new(rail.width() - PAD * 2.0, 40.0),
        );
        let response = ui.interact(rect, ui.make_persistent_id("nav-connect"), Sense::click());
        let painter = ui.painter().clone();
        if response.hovered() {
            painter.rect_filled(rect, t::R_SM, t::INK_2);
        }
        painter.rect_stroke(
            rect,
            t::R_SM,
            Stroke::new(
                1.0,
                if response.hovered() {
                    accent.deep
                } else {
                    t::LINE_3
                },
            ),
            egui::StrokeKind::Inside,
        );

        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 16.0, rect.top() + 14.0),
            Vec2::splat(13.0),
        );
        // The plus is composed from two strokes rather than an icon function, so
        // the dashed slot stays visually quieter than the items above it.
        painter.line_segment(
            [
                Pos2::new(icon_rect.center().x - 5.0, icon_rect.center().y),
                Pos2::new(icon_rect.center().x + 5.0, icon_rect.center().y),
            ],
            Stroke::new(1.6, t::FG_3),
        );
        painter.line_segment(
            [
                Pos2::new(icon_rect.center().x, icon_rect.center().y - 5.0),
                Pos2::new(icon_rect.center().x, icon_rect.center().y + 5.0),
            ],
            Stroke::new(1.6, t::FG_3),
        );

        widgets::label_at(
            ui,
            Pos2::new(rect.left() + 28.0, rect.top() + 14.0),
            Align2::LEFT_CENTER,
            "接入来源",
            t::ui_font(12.0),
            t::FG_2,
        );
        widgets::engraved_at(
            ui,
            Pos2::new(rect.left() + 10.0, rect.top() + 29.0),
            Align2::LEFT_CENTER,
            "关键词搜索 · 直链解析 · 歌词 · 会话",
            8.5,
            t::FG_4,
        );

        if response.clicked() {
            self.open_connect_sheet();
        }
    }
}

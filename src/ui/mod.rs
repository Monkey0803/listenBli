//! Shared UI: the shell (top bar, list pane, status line), the reusable track
//! row, and the small buttons the panes share.

pub mod icons;
pub mod library;
pub mod login;
pub mod lyrics;
pub mod player;
pub mod settings;
pub mod theme;
pub mod widgets;

use egui::{
    Align, Align2, Color32, CursorIcon, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    TextEdit, Ui, Vec2,
};

use crate::api::models::Track;
use crate::util;

use self::icons::Icon;
use self::theme::{self as t, Accent};
use self::widgets::{chip, Badge};
use super::app::{App, Tab};

/// Panel backgrounds, so the shell and the panels agree on one surface.
pub mod frames {
    use super::theme;
    use egui::{Color32, CornerRadius, Frame, Margin};

    pub fn top_bar() -> Frame {
        Frame::NONE
            .fill(theme::INK_1)
            .inner_margin(Margin::symmetric(16, 0))
    }

    pub fn status_bar() -> Frame {
        Frame::NONE
            .fill(Color32::from_black_alpha(87))
            .inner_margin(Margin::symmetric(18, 0))
    }

    pub fn player_bar(_theme: &theme::Theme) -> Frame {
        Frame::NONE
            .fill(Color32::from_rgba_unmultiplied(8, 8, 12, 184))
            .inner_margin(Margin {
                left: 22,
                right: 22,
                top: 12,
                bottom: 12,
            })
    }

    pub fn lyrics(_theme: &theme::Theme) -> Frame {
        Frame::NONE
            .fill(theme::INK_1)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::ZERO)
    }

    pub fn list_pane() -> Frame {
        Frame::NONE.fill(theme::INK_1).inner_margin(Margin::ZERO)
    }
}

/// A 1px separator, drawn on the boundary of the panel that owns it.
fn hline(ui: &Ui, y: f32, color: Color32) {
    let rect = ui.max_rect();
    ui.painter()
        .hline(rect.x_range(), y, Stroke::new(1.0, color));
}

// ---------------------------------------------------------------------------
// small buttons
// ---------------------------------------------------------------------------

/// The design's `.btn-ghost`: a bordered, low-emphasis action.
pub(crate) fn ghost_button(
    ui: &mut Ui,
    label: &str,
    icon: Option<Icon>,
    accent: Accent,
    accent_style: bool,
    enabled: bool,
) -> egui::Response {
    let font = t::ui_font(12.5);
    let text_w = if label.is_empty() {
        0.0
    } else {
        widgets::measure(ui, label, &font).x
    };
    let icon_w = if icon.is_some() { 20.0 } else { 0.0 };
    let width = text_w + icon_w + 24.0;

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered() && enabled;

    let (fill, stroke, fg) = if accent_style && enabled {
        (
            if hovered {
                t::fade(accent.accent, 0.32)
            } else {
                accent.dim()
            },
            Stroke::new(1.0, t::fade(accent.accent, 0.34)),
            if hovered { Color32::WHITE } else { accent.ink },
        )
    } else {
        (
            if hovered {
                t::white(0.06)
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, if hovered { t::LINE_2 } else { t::LINE }),
            if !enabled {
                t::FG_4
            } else if hovered {
                t::FG
            } else {
                t::FG_2
            },
        )
    };
    painter.rect_filled(rect, t::R_SM, fill);
    painter.rect_stroke(rect, t::R_SM, stroke, egui::StrokeKind::Inside);

    let mut cursor = rect.left() + 12.0;
    if let Some(icon) = icon {
        icon(
            &painter,
            Rect::from_min_size(Pos2::new(cursor, rect.center().y - 6.0), Vec2::splat(12.0)),
            fg,
        );
        cursor += 20.0;
    }
    if !label.is_empty() {
        widgets::paint_label(
            &painter,
            Pos2::new(cursor, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            fg,
        );
    }
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

/// The design's `.btn-primary`: the one accent-filled action per screen.
pub(crate) fn primary_button(
    ui: &mut Ui,
    label: &str,
    icon: Option<Icon>,
    accent: Accent,
) -> egui::Response {
    let font = t::ui_font(13.0);
    let text_w = widgets::measure(ui, label, &font).x;
    let icon_w = if icon.is_some() { 23.0 } else { 0.0 };
    let width = text_w + icon_w + 36.0;

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered();
    t::gradient_rounded_rect(
        &painter,
        rect,
        t::R_MD,
        accent.soft,
        if hovered {
            t::mix(accent.soft, accent.accent, 0.6)
        } else {
            accent.accent
        },
        Vec2::new(0.0, 1.0),
    );
    painter.rect_stroke(
        rect,
        t::R_MD,
        Stroke::new(1.0, t::white(0.22)),
        egui::StrokeKind::Inside,
    );

    let ink = Color32::from_rgb(0x2a, 0x0d, 0x18);
    let mut cursor = rect.left() + 18.0;
    if let Some(icon) = icon {
        icon(
            &painter,
            Rect::from_min_size(Pos2::new(cursor, rect.center().y - 7.5), Vec2::splat(15.0)),
            ink,
        );
        cursor += 23.0;
    }
    widgets::paint_label(
        &painter,
        Pos2::new(cursor, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        ink,
    );
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

/// The centred header every list pane shares: a big title, a meta count and
/// right-aligned actions, closed by a hairline.
pub(crate) fn list_header(ui: &mut Ui, title: &str, meta: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        ui.vertical(|ui| {
            ui.add_space(17.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(19.0).color(t::FG));
                if !meta.is_empty() {
                    ui.label(RichText::new(meta).size(12.0).color(t::FG_3));
                }
            });
            ui.add_space(14.0);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(22.0);
            actions(ui);
        });
    });
    hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);
}

// ---------------------------------------------------------------------------
// top bar
// ---------------------------------------------------------------------------

impl App {
    pub(crate) fn ui_top_bar(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let playing = self.is_playing();
        let logged_in = self.user.is_some();

        let rect = ui.max_rect();
        hline(ui, rect.bottom() - 0.5, t::LINE);

        ui.horizontal_centered(|ui| {
            self.brand(ui, accent, playing);
            ui.add_space(8.0);
            if let Some(tab) = self.tab_control(ui, logged_in) {
                self.tab = tab;
            }
            ui.add_space(8.0);
            self.search_field(ui);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.account_control(ui, accent);

                ui.add_space(4.0);
                let active = self.settings_open;
                if widgets::icon_button(ui, icons::settings, 32.0, active, accent, "设置").clicked()
                {
                    self.settings_open = !self.settings_open;
                    self.account_menu_open = false;
                }

                ui.add_space(4.0);
                let flac = self.config_value(|c| c.prefer_flac);
                if chip(ui, "无损优先", Some(icons::spark), None, flac, accent).clicked() {
                    let vip = self.user.as_ref().is_some_and(|u| u.is_vip());
                    self.update_config(|config| config.prefer_flac = !flac);
                    let message = if flac {
                        "已关闭无损优先".to_owned()
                    } else if vip {
                        "已开启无损优先（需投稿提供 FLAC，否则回落 192K）".to_owned()
                    } else {
                        "已开启无损优先：登录大会员账号后才会生效".to_owned()
                    };
                    self.set_status(message, false);
                }
            });
        });
    }

    /// The animated brand mark and wordmark.
    fn brand(&self, ui: &mut Ui, accent: Accent, playing: bool) {
        let (mark, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
        let painter = ui.painter().clone();
        t::gradient_rounded_rect(
            &painter,
            mark,
            8.0,
            accent.accent,
            accent.deep,
            Vec2::new(0.6, 0.9),
        );
        let bars = Rect::from_min_size(
            Pos2::new(mark.left() + 6.0, mark.top() + 5.0),
            Vec2::new(mark.width() - 12.0, mark.height() - 11.0),
        );
        let time = ui.input(|i| i.time) as f32;
        let mut scale = [0.58, 1.0, 0.75];
        if playing {
            for (index, value) in scale.iter_mut().enumerate() {
                let phase = time * 1.05 * std::f32::consts::TAU + index as f32 * 0.7;
                *value = (0.75 + 0.42 * phase.sin()).clamp(0.3, 1.0);
            }
        }
        icons::brand_bars(
            &painter,
            bars,
            Color32::from_rgba_unmultiplied(255, 255, 255, 242),
            scale,
        );

        ui.add_space(4.0);
        let font = t::ui_font(16.5);
        let head = widgets::measure(ui, "listen", &font).x;
        let tail = widgets::measure(ui, "Bli", &font).x;
        let (word, _) = ui.allocate_exact_size(Vec2::new(head + tail, 22.0), Sense::hover());
        widgets::label_at_bold(
            ui,
            word.left_center(),
            Align2::LEFT_CENTER,
            "listen",
            font.clone(),
            t::FG,
        );
        widgets::label_at_bold(
            ui,
            Pos2::new(word.left() + head, word.center().y),
            Align2::LEFT_CENTER,
            "Bli",
            font,
            accent.accent,
        );
    }

    /// The three-way tab control from the design's `.segmented`.
    fn tab_control(&mut self, ui: &mut Ui, logged_in: bool) -> Option<Tab> {
        let accent = self.theme.accent;
        let fav_count = self
            .fav_folders
            .iter()
            .find(|f| Some(f.id) == self.selected_folder)
            .map(|f| f.media_count)
            .unwrap_or(self.fav_items.len() as i64);
        let history_count = self.history.len();

        let entries: [(Tab, &str, Icon, Option<String>); 3] = [
            (Tab::Search, "搜索", icons::search, None),
            (
                Tab::Favorites,
                "收藏夹",
                icons::folder,
                logged_in.then(|| fav_count.to_string()),
            ),
            (
                Tab::History,
                "历史",
                icons::clock,
                logged_in.then(|| history_count.to_string()),
            ),
        ];

        let font = t::ui_font(12.5);
        let mut widths = [0.0f32; 3];
        for (index, (_, label, _, count)) in entries.iter().enumerate() {
            let count_w = count
                .as_ref()
                .map(|c| widgets::measure(ui, c, t::mono_font(10.5)).x + 12.0)
                .unwrap_or(0.0);
            widths[index] = widgets::measure(ui, label, &font).x + 13.0 + 6.0 + count_w + 24.0;
        }
        let total: f32 = widths.iter().sum::<f32>() + 6.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 32.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_filled(rect, t::R_MD, t::white(0.045));
        painter.rect_stroke(
            rect,
            t::R_MD,
            Stroke::new(1.0, t::LINE),
            egui::StrokeKind::Inside,
        );

        let mut picked = None;
        let mut cursor = rect.left() + 3.0;
        for (index, (tab, label, icon, count)) in entries.iter().enumerate() {
            let cell = Rect::from_min_size(
                Pos2::new(cursor, rect.top() + 3.0),
                Vec2::new(widths[index], 26.0),
            );
            let response = ui.interact(cell, ui.id().with(("tab", index)), Sense::click());
            let active = self.tab == *tab;
            if active {
                painter.rect_filled(cell, 7.0, t::INK_4);
                painter.rect_stroke(
                    cell,
                    7.0,
                    Stroke::new(1.0, t::white(0.06)),
                    egui::StrokeKind::Inside,
                );
            }
            let fg = if active {
                t::FG
            } else if response.hovered() {
                t::FG_2
            } else {
                t::FG_3
            };

            let icon_rect = Rect::from_min_size(
                Pos2::new(cell.left() + 12.0, cell.center().y - 6.5),
                Vec2::splat(13.0),
            );
            if logged_in {
                icon(&painter, icon_rect, fg);
            } else {
                icons::lock(&painter, icon_rect, t::FG_3);
            }
            let mut x = cell.left() + 12.0 + 19.0;
            widgets::label_at(
                ui,
                Pos2::new(x, cell.center().y),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                fg,
            );
            x += widgets::measure(ui, label, font.clone()).x;

            if let Some(count) = count {
                let w = widgets::measure(ui, count, t::mono_font(10.5)).x + 12.0;
                let badge = Rect::from_min_size(
                    Pos2::new(x + 6.0, cell.center().y - 7.5),
                    Vec2::new(w, 15.0),
                );
                let (bg, text) = if active {
                    (accent.dim(), accent.ink)
                } else {
                    (t::white(0.06), t::FG_3)
                };
                painter.rect_filled(badge, 7.5, bg);
                widgets::paint_label(
                    &painter,
                    badge.center(),
                    Align2::CENTER_CENTER,
                    count,
                    t::mono_font(10.5),
                    text,
                );
            }

            if response.clicked() {
                picked = Some(*tab);
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            cursor += widths[index];
        }
        picked
    }

    /// The ⌘K search pill. The frame paints behind its own contents, which is
    /// what keeps the text editable and visible at the same time.
    fn search_field(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let cleared = self.search_input.is_empty();
        let mut focus_requested = false;
        // Named so the focus state is known *before* the field is built: the
        // icon and the fill change in the same frame the ring appears.
        let field_id = egui::Id::new("search-field");
        let focused = ui.ctx().memory(|memory| memory.focused()) == Some(field_id);

        let frame = egui::Frame::NONE
            .fill(Color32::from_black_alpha(if focused { 140 } else { 107 }))
            .stroke(Stroke::new(1.0, t::LINE))
            .corner_radius(18.0)
            .inner_margin(Margin {
                left: 12,
                right: 10,
                top: 0,
                bottom: 0,
            });

        let inner = frame.show(ui, |ui| {
            // Both bounds on both axes: the field is a 318x36 pill, and every
            // child that fills "the rest" would otherwise grow it. Height: the
            // text edit's growing atom stretches the frame to the whole top
            // bar, turning the pill into a bar-height slab. Width: the
            // right-to-left run below lays the ⌘K chip against the end of
            // whatever rect it is given, so an unbounded pill swallowed the
            // 无损优先 / 设置 / 账号 cluster — the fill and the `focus-within`
            // ring wrapped them, and the chip ended up under the account chip.
            ui.set_width(318.0);
            ui.set_height(36.0);

            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(15.0, 36.0), Sense::hover());
            icons::search(
                ui.painter(),
                Rect::from_center_size(icon_rect.center(), Vec2::splat(15.0)),
                if focused { accent.accent } else { t::FG_4 },
            );
            ui.add_space(9.0);

            let font = t::ui_font(13.0);
            let offset = widgets::field_text_offset(ui.painter(), &font);
            let text_color = t::FG;
            let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                widgets::field_galley(ui, buf.as_str(), &font, text_color, offset, wrap)
            };
            let edit = ui.add(
                TextEdit::singleline(&mut self.search_input)
                    .id(field_id)
                    .frame(egui::Frame::NONE)
                    // The default is LEFT_TOP, which floats the text above the
                    // middle of the pill.
                    .vertical_align(Align::Center)
                    .layouter(&mut layouter)
                    .font(font.clone())
                    .text_color(text_color)
                    .desired_width(230.0)
                    .margin(Margin::ZERO),
            );
            // Painted rather than set as `hint_text`, so the placeholder shares
            // the input text's centring instead of egui's own.
            if self.search_input.is_empty() {
                widgets::paint_label(
                    ui.painter(),
                    edit.rect.left_center(),
                    Align2::LEFT_CENTER,
                    "搜索歌曲 / 视频，如：周杰伦 晴天",
                    font,
                    t::FG_3,
                );
            }
            if self.focus_search {
                edit.request_focus();
                self.focus_search = false;
                focus_requested = true;
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if cleared {
                    let (chip_rect, _) =
                        ui.allocate_exact_size(Vec2::new(30.0, 18.0), Sense::hover());
                    let painter = ui.painter().clone();
                    painter.rect_filled(chip_rect, 5.0, t::white(0.055));
                    painter.rect_stroke(
                        chip_rect,
                        5.0,
                        Stroke::new(1.0, t::LINE),
                        egui::StrokeKind::Inside,
                    );
                    widgets::paint_label(
                        &painter,
                        chip_rect.center(),
                        Align2::CENTER_CENTER,
                        "⌘K",
                        t::mono_font(10.5),
                        t::FG_3,
                    );
                } else if widgets::icon_button(ui, icons::close, 24.0, false, accent, "清空")
                    .clicked()
                {
                    self.search_input.clear();
                }
            });

            edit
        });

        if focused {
            let painter = ui.painter().clone();
            // The design's `focus-within`: a 1px accent ring with a 4px halo
            // flush against it. Expanding the halo (or raising its radius) left
            // a gap that read as a doubled, broken outline.
            painter.rect_stroke(
                inner.response.rect,
                18.0,
                Stroke::new(1.0, t::fade(accent.accent, 0.55)),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                inner.response.rect,
                18.0,
                Stroke::new(4.0, accent.dim()),
                egui::StrokeKind::Outside,
            );
        }

        let submitted =
            inner.inner.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if submitted && !self.search_input.trim().is_empty() {
            self.start_search();
        }

        // The dropdown follows the field: it appears when the empty field takes
        // focus and steps aside as soon as there is a query to run.
        if !self.search_input.is_empty() {
            self.search_history_open = false;
        } else if focus_requested || inner.inner.gained_focus() {
            self.search_history_open = true;
        }
        if self.search_history_open {
            self.search_history_popup(&inner.response);
            // egui's `CloseOnClickOutside` forgives clicks that land on the
            // popup, but not the mouse release that focuses the field: that one
            // counts as clicking away and dismissed the list the frame after it
            // appeared. A click on the field itself is not clicking away, so put
            // the list back. (⌘K never suffered from this: it is not a click.)
            if inner.inner.clicked()
                && self.search_input.is_empty()
                && !self.search_history().is_empty()
            {
                self.search_history_open = true;
            }
        }
    }

    /// The recent-search dropdown: the last few queries, click to run one again.
    fn search_history_popup(&mut self, anchor: &egui::Response) {
        let history = self.search_history();
        if history.is_empty() {
            self.search_history_open = false;
            return;
        }

        let mut picked: Option<String> = None;
        let mut clear = false;
        egui::Popup::from_response(anchor)
            .open_bool(&mut self.search_history_open)
            .align(egui::emath::RectAlign::BOTTOM_START)
            .gap(8.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_3)
                    .corner_radius(t::R_LG)
                    .inner_margin(Margin::same(6))
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .show(|ui| {
                ui.set_min_width(anchor.rect.width().max(240.0));
                ui.vertical(|ui| {
                    ui.add_space(7.0);
                    ui.label(RichText::new("搜索记录").size(11.0).color(t::FG_4));
                    ui.add_space(3.0);
                    for keyword in &history {
                        if menu_item(ui, keyword, icons::clock, false) {
                            picked = Some(keyword.clone());
                        }
                    }
                    ui.add_space(6.0);
                    hline(ui, ui.min_rect().bottom(), t::LINE);
                    ui.add_space(6.0);
                    if menu_item(ui, "清空搜索记录", icons::close, true) {
                        clear = true;
                    }
                });
            });

        if let Some(keyword) = picked {
            self.search_input = keyword;
            self.start_search();
        }
        if clear {
            self.clear_search_history();
        }
    }

    pub(crate) fn start_search(&mut self) {
        let keyword = self.search_input.trim().to_owned();
        if keyword.is_empty() {
            return;
        }
        self.tab = Tab::Search;
        self.search_page = 1;
        self.search_has_more = false;
        self.loading_more = false;
        // Every search the user actually runs is remembered, whichever control
        // started it (Enter, ⌘K, a suggestion chip or the history dropdown).
        self.remember_search(&keyword);
        self.send(crate::net::Cmd::Search { keyword, page: 1 });
    }

    /// Signed-in account chip with its dropdown, or the login prompt.
    fn account_control(&mut self, ui: &mut Ui, accent: Accent) {
        let Some(user) = self.user.clone() else {
            if chip(ui, "扫码登录", Some(icons::qr), None, false, accent).clicked() {
                self.open_login();
            }
            return;
        };

        let font = t::ui_font(12.5);
        let name_w = widgets::measure(ui, &user.uname, &font).x;
        let vip_w = if user.is_vip() { 44.0 } else { 0.0 };
        let width = 4.0 + 26.0 + 8.0 + name_w + vip_w + 8.0 + 13.0 + 10.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
        let painter = ui.painter().clone();
        let hovered = response.hovered();

        painter.rect_filled(
            rect,
            rect.height() * 0.5,
            t::white(if hovered { 0.075 } else { 0.035 }),
        );
        painter.rect_stroke(
            rect,
            rect.height() * 0.5,
            Stroke::new(1.0, if hovered { t::LINE_2 } else { t::LINE }),
            egui::StrokeKind::Inside,
        );

        let avatar = Rect::from_center_size(
            Pos2::new(rect.left() + 17.0, rect.center().y),
            Vec2::splat(26.0),
        );
        t::gradient_rounded_rect(
            &painter,
            avatar,
            13.0,
            Color32::from_rgb(0x5f, 0x6b, 0xd8),
            Color32::from_rgb(0x8d, 0x4f, 0xb8),
            Vec2::new(0.6, 0.9),
        );
        widgets::paint_label(
            &painter,
            avatar.center(),
            Align2::CENTER_CENTER,
            user.uname.chars().next().unwrap_or('?').to_string(),
            t::ui_font(12.0),
            Color32::WHITE,
        );

        let mut x = avatar.right() + 8.0;
        widgets::label_at(
            ui,
            Pos2::new(x, rect.center().y),
            Align2::LEFT_CENTER,
            &user.uname,
            font,
            t::FG,
        );
        x += name_w;
        if user.is_vip() {
            let badge = Rect::from_min_size(
                Pos2::new(x + 4.0, rect.center().y - 8.0),
                Vec2::new(vip_w - 6.0, 16.0),
            );
            painter.rect_filled(badge, 4.0, t::GOLD);
            widgets::paint_label(
                &painter,
                badge.center(),
                Align2::CENTER_CENTER,
                "大会员",
                t::ui_font(10.0),
                Color32::from_rgb(0x2b, 0x1a, 0x06),
            );
            x += vip_w;
        }
        icons::chevron(
            &painter,
            Rect::from_min_size(Pos2::new(x + 4.0, rect.center().y - 6.5), Vec2::splat(13.0)),
            t::FG_4,
        );

        if hovered {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            self.account_menu_open = !self.account_menu_open;
        }

        let mut logout = false;
        let mut open_settings = false;
        egui::Popup::from_response(&response)
            .open(self.account_menu_open)
            .align(egui::emath::RectAlign::BOTTOM_END)
            .gap(8.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_3)
                    .corner_radius(t::R_LG)
                    .inner_margin(Margin::same(6))
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .show(|ui| {
                ui.set_min_width(196.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new(&user.uname).size(13.0).color(t::FG));
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("UID {}", user.mid))
                            .font(t::mono_font(11.0))
                            .color(t::FG_3),
                    );
                    ui.add_space(8.0);
                    hline(ui, ui.min_rect().bottom(), t::LINE);
                    ui.add_space(6.0);
                    if menu_item(ui, "播放设置", icons::settings, false) {
                        open_settings = true;
                    }
                    if menu_item(ui, "退出登录", icons::close, true) {
                        logout = true;
                    }
                });
            });

        if open_settings {
            self.settings_open = true;
            self.account_menu_open = false;
        }
        if logout {
            self.account_menu_open = false;
            self.send(crate::net::Cmd::Logout);
        }
    }

    pub(crate) fn open_login(&mut self) {
        self.settings_open = false;
        self.account_menu_open = false;
        self.send(crate::net::Cmd::QrStart);
        self.qr = Some(super::app::QrDialog {
            key: String::new(),
            image: None,
            texture: None,
            message: "请使用哔哩哔哩手机客户端扫码".into(),
            last_poll: std::time::Instant::now(),
            finished: false,
        });
    }
}

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
        painter.rect_filled(
            rect,
            t::R_SM,
            if danger {
                t::fade(t::ERR, 0.18)
            } else {
                t::white(0.07)
            },
        );
    }
    icon(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 10.0, rect.center().y - 7.0),
            Vec2::splat(14.0),
        ),
        fg,
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 33.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        t::ui_font(12.5),
        fg,
    );
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response.clicked()
}

// ---------------------------------------------------------------------------
// central pane
// ---------------------------------------------------------------------------

impl App {
    pub(crate) fn ui_central(&mut self, ui: &mut Ui) {
        match self.tab {
            Tab::Search => self.ui_search_pane(ui),
            Tab::Favorites => self.ui_favorites(ui),
            Tab::History => self.ui_history(ui),
        }
    }

    fn ui_search_pane(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let searching = self.searching;
        let keyword = self.last_keyword.clone();
        let count = self.results.len();

        let title = if searching {
            format!("搜索「{keyword}」")
        } else if keyword.is_empty() {
            "搜索".to_owned()
        } else {
            format!("「{keyword}」")
        };
        let meta = if !searching && count > 0 {
            format!("找到 {count} 个结果")
        } else {
            String::new()
        };

        let mut play_all = false;
        list_header(ui, &title, &meta, |ui| {
            if searching {
                spinner(ui, 15.0, accent);
            } else if count > 0
                && ghost_button(ui, "全部播放", Some(icons::play), accent, true, true).clicked()
            {
                play_all = true;
            }
        });

        if play_all {
            let results = self.results.clone();
            self.play_from_list(&results, 0);
            return;
        }

        if searching {
            widgets::skeleton(ui, 6, self.theme.row_h);
            return;
        }

        if self.results.is_empty() {
            let examples: &[&str] = &["周杰伦 晴天", "米津玄師", "七里香"];
            // The user's own recent searches are more useful than the built-in
            // examples, but a first run still needs somewhere to start from.
            let history = self.search_history();
            let recent: Vec<&str> = history.iter().map(String::as_str).collect();
            let (desc, suggestions): (&str, &[&str]) = if keyword.is_empty() && !recent.is_empty() {
                (
                    "下面是最近的搜索记录，点击即可重新搜索；也可以直接输入新的关键词。",
                    &recent,
                )
            } else {
                (
                    if keyword.is_empty() {
                        "listenBli 会把 B 站上的音乐/视频下载成音轨后直接播放，不需要登录即可获得 192K 音质。"
                    } else {
                        "试试换一个关键词，或者用「歌手 + 歌名」的形式，例如「周杰伦 晴天」或「米津玄師 Lemon」。"
                    },
                    examples,
                )
            };
            let empty_title = if keyword.is_empty() {
                "输入关键词开始搜索".to_owned()
            } else {
                format!("没有找到「{keyword}」相关的结果")
            };
            let picked =
                widgets::empty_state(ui, icons::search, &empty_title, desc, suggestions, accent);
            if let Some(suggestion) = picked {
                self.search_input = suggestion;
                self.start_search();
            }
            return;
        }

        self.pane_body(ui, PaneSource::Search);
    }

    /// The scrollable body shared by search, favourites and history.
    pub(crate) fn pane_body(&mut self, ui: &mut Ui, source: PaneSource) {
        let accent = self.theme.accent;
        let mut clicked = None;
        let mut load_more = false;

        let tracks: Vec<Track> = match source {
            PaneSource::Search => std::mem::take(&mut self.results),
            PaneSource::Favorites => std::mem::take(&mut self.fav_items),
            PaneSource::History => std::mem::take(&mut self.history),
        };
        let has_more = match source {
            PaneSource::Search => self.search_has_more,
            PaneSource::Favorites => self.fav_has_more,
            PaneSource::History => false,
        };

        let scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                column_head(ui, 24.0);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        for (index, track) in tracks.iter().enumerate() {
                            if let Some(action) = track_row(ui, self, track, index) {
                                clicked = Some((index, action));
                            }
                            ui.add_space(1.0);
                        }
                    });
                });
                ui.add_space(18.0);
                // Paging has no button: a page still in flight only shows this
                // spinner, and reaching the tail of the list is what asks for
                // the next one (see below).
                if self.loading_more {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        spinner(ui, 15.0, accent);
                    });
                }
                ui.add_space(22.0);
            });

        match source {
            PaneSource::Search => self.results = tracks,
            PaneSource::Favorites => self.fav_items = tracks,
            PaneSource::History => self.history = tracks,
        }

        if let Some((index, action)) = clicked {
            let list = match source {
                PaneSource::Search => self.results.clone(),
                PaneSource::Favorites => self.fav_items.clone(),
                PaneSource::History => self.history.clone(),
            };
            let track = list.get(index).cloned();
            match action {
                RowAction::Play => self.play_from_list(&list, index),
                RowAction::CopyLink => {
                    if let Some(track) = &track {
                        ui.ctx().copy_text(util::video_url(&track.bvid));
                        self.notify("已复制视频链接");
                    }
                }
                RowAction::OpenInBrowser => {
                    if let Some(track) = &track {
                        if let Err(err) = crate::platform::open_url(&util::video_url(&track.bvid)) {
                            self.set_status(err, true);
                        }
                    }
                }
                RowAction::Enqueue | RowAction::EnqueueNext => {
                    let next = action == RowAction::EnqueueNext;
                    if let Some(track) = track {
                        let title = track.title.clone();
                        if self.enqueue(track, next) {
                            self.notify(if next {
                                format!("已排在下一首：{title}")
                            } else {
                                format!("已加入播放列表：{title}")
                            });
                        } else {
                            self.notify("已经在播放列表里了");
                        }
                    }
                }
                RowAction::Cache => {
                    if let Some(track) = track {
                        let title = track.title.clone();
                        if self.cache_track(&track) {
                            self.notify(format!("正在缓存：{title}"));
                        } else {
                            self.notify("这首正在播放，音频已经在缓存了");
                        }
                    }
                }
                RowAction::Export => {
                    if let Some(track) = track {
                        // Implies caching: the worker downloads first if it has
                        // to, then writes the file. Both take time, so the toast
                        // reports the start and the event reports the result.
                        let title = track.title.clone();
                        self.send(crate::net::Cmd::ExportTrack(Box::new(track)));
                        self.notify(format!("正在导出：{title}"));
                    }
                }
            }
        }

        // Scrolling to the end fetches the next page by itself, so the rows are
        // usually already laid out by the time the user gets there. A list
        // shorter than its viewport counts as "at the end" too: there is
        // nothing left to scroll, so waiting for a scroll would deadlock it.
        if has_more
            && !self.loading_more
            && near_end(
                scroll.state.offset.y,
                scroll.inner_rect.height(),
                scroll.content_size.y,
            )
        {
            load_more = true;
        }

        if load_more {
            // Marked in flight *before* the command leaves, so the next frame
            // cannot ask for the same page while the worker is still busy.
            match source {
                // A later page of the query already on screen.
                PaneSource::Search => {
                    self.loading_more = true;
                    self.send(crate::net::Cmd::Search {
                        keyword: self.last_keyword.clone(),
                        page: self.search_page + 1,
                    });
                }
                PaneSource::Favorites => {
                    if let Some(media_id) = self.selected_folder {
                        self.loading_more = true;
                        let page = self.fav_page + 1;
                        self.send(crate::net::Cmd::LoadFavItems { media_id, page });
                    }
                }
                PaneSource::History => {}
            }
        }
    }
}

/// How close to the end of the list (in points) the next page is fetched. About
/// the height of four rows, which leaves the in-flight spinner off screen until
/// the user scrolls into it.
const AUTO_LOAD_MARGIN: f32 = 320.0;

/// Whether a list is close enough to its end to fetch more. `offset` is the
/// vertical scroll offset, `viewport` the height of the visible window and
/// `content` the height of everything laid out inside it.
fn near_end(offset: f32, viewport: f32, content: f32) -> bool {
    content - (offset + viewport) <= AUTO_LOAD_MARGIN
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneSource {
    Search,
    Favorites,
    History,
}

/// The design's `.colhead`: uppercase micro-labels above the columns.
pub(crate) fn column_head(ui: &mut Ui, side_pad: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let painter = ui.painter().clone();
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + side_pad + 102.0, rect.center().y),
        Align2::LEFT_CENTER,
        "标题",
        t::ui_font(10.0),
        t::FG_3,
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.right() - side_pad - 28.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "音质 / 时长",
        t::ui_font(10.0),
        t::FG_3,
    );
}

fn spinner(ui: &mut Ui, size: f32, accent: Accent) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter().clone();
    let time = ui.input(|i| i.time) as f32;
    let start = (time * 4.2) % std::f32::consts::TAU;
    let points: Vec<Pos2> = (0..=20)
        .map(|i| {
            let angle = start + i as f32 / 20.0 * std::f32::consts::TAU * 0.78;
            rect.center() + Vec2::angled(angle) * (size * 0.5 - 1.0)
        })
        .collect();
    painter.line(points, Stroke::new(1.8, accent.accent));
    ui.ctx().request_repaint();
}

/// What a row was asked to do. The row cannot run any of it itself: it is painted
/// while the list is borrowed out of `App`, so the click travels back to the pane
/// that owns the `&mut App`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowAction {
    /// The row body: make this the current track and start it.
    Play,
    /// `⋯` menu: put the video's page URL on the clipboard.
    CopyLink,
    /// `⋯` menu: hand that URL to the system browser.
    OpenInBrowser,
    /// `⋯` menu: append to the play queue.
    Enqueue,
    /// `⋯` menu: play it right after the current track.
    EnqueueNext,
    /// `⋯` menu: download it into the audio cache without playing it.
    Cache,
    /// `⋯` menu: write a normal `.m4a` copy of the audio into the export folder.
    Export,
}

/// One row of the list. Clicking anywhere on it plays that track; the `⋯` button
/// opens the per-row menu instead, and both come back as a [`RowAction`].
pub(crate) fn track_row(ui: &mut Ui, app: &App, track: &Track, index: usize) -> Option<RowAction> {
    let accent = app.theme.accent;
    let row_h = app.theme.row_h;
    let compact = app.theme.compact();
    let current = app.is_current(track);
    let playing = app.is_playing();

    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered();

    if current {
        t::gradient_rounded_rect(
            &painter,
            rect,
            t::R_MD,
            accent.dim(),
            t::white(0.022),
            Vec2::new(1.0, 0.0),
        );
    } else if hovered {
        painter.rect_filled(rect, t::R_MD, t::INK_3);
    }

    let bar_h = if current {
        34.0
    } else if hovered {
        20.0
    } else {
        0.0
    };
    if bar_h > 0.0 {
        painter.rect_filled(
            Rect::from_center_size(
                Pos2::new(rect.left() + 1.25, rect.center().y),
                Vec2::new(2.5, bar_h),
            ),
            2.0,
            accent.accent,
        );
    }

    let inner = rect.shrink2(Vec2::new(10.0, 0.0));

    let idx_rect = Rect::from_min_size(Pos2::new(inner.left(), rect.top()), Vec2::new(28.0, row_h));
    if current {
        // The loaded row says so here instead of showing its number: the tint,
        // the side bar and the accent title all carry "this one is current", so
        // the one part of the row that could say *playing* is this column.
        //
        // A pause glyph, matching what the cover overlay already paints for
        // `current && playing`. Deliberately not keyed on the paused state: a
        // paused row would then fall back to a play triangle, which is the thing
        // this replaced.
        icons::pause(
            &painter,
            Rect::from_center_size(idx_rect.center(), Vec2::splat(15.0)),
            accent.accent,
        );
    } else {
        widgets::paint_label(
            &painter,
            idx_rect.center(),
            Align2::CENTER_CENTER,
            format!("{:02}", index + 1),
            t::mono_font(11.5),
            t::FG_3,
        );
    }

    let cover_size = if compact { 38.0 } else { 46.0 };
    let cover_rect = Rect::from_center_size(
        Pos2::new(idx_rect.right() + 14.0 + cover_size * 0.5, rect.center().y),
        Vec2::splat(cover_size),
    );
    widgets::paint_cover(
        &painter,
        cover_rect,
        Some(track),
        app.covers.get(&track.key()),
    );
    if hovered || current {
        let overlay = Rect::from_center_size(
            cover_rect.center(),
            cover_rect.size() * if hovered { 1.04 } else { 1.0 },
        );
        painter.rect_filled(
            overlay,
            9.0,
            Color32::from_black_alpha(if current { 140 } else { 158 }),
        );
        if current && playing {
            widgets::equalizer(
                ui,
                Rect::from_center_size(overlay.center(), Vec2::new(16.0, 15.0)),
                playing,
                accent.accent,
            );
        } else {
            icons::play(
                &painter,
                Rect::from_center_size(
                    overlay.center() + Vec2::new(1.0, 0.0),
                    Vec2::splat(if compact { 13.0 } else { 15.0 }),
                ),
                Color32::WHITE,
            );
        }
    }

    let more_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 10.0 - 16.0, rect.center().y - 8.0),
        Vec2::splat(16.0),
    );
    let dur_rect = Rect::from_min_size(
        Pos2::new(more_rect.left() - 12.0 - 42.0, rect.top()),
        Vec2::new(42.0, row_h),
    );
    let badge_rect = Rect::from_min_size(
        Pos2::new(dur_rect.left() - 12.0 - 62.0, rect.top()),
        Vec2::new(62.0, row_h),
    );

    widgets::paint_label(
        &painter,
        dur_rect.center(),
        Align2::CENTER_CENTER,
        util::format_duration(track.duration),
        t::mono_font(11.5),
        t::FG_3,
    );
    // The `⋯` is its own widget, so clicking it opens the menu instead of
    // playing: registered after the row, it wins the hit test on its own rect.
    let more = ui.interact(
        more_rect.expand(5.0),
        ui.make_persistent_id(("track-more", track.key())),
        Sense::click(),
    );
    if hovered || more.hovered() || more.has_focus() {
        painter.rect_filled(more_rect.expand(4.0), t::R_SM, t::white(0.07));
        icons::more(&painter, more_rect, t::FG);
    }
    if more.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let mut action = None;
    egui::Popup::menu(&more)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .align(egui::emath::RectAlign::BOTTOM_END)
        .gap(4.0)
        .frame(
            egui::Frame::NONE
                .fill(t::INK_3)
                .corner_radius(t::R_LG)
                .inner_margin(Margin::same(5))
                .stroke(Stroke::new(1.0, t::LINE_2)),
        )
        .show(|ui| {
            ui.set_min_width(186.0);
            if menu_item(ui, "复制视频链接", icons::copy, false) {
                action = Some(RowAction::CopyLink);
            }
            if menu_item(ui, "在浏览器打开", icons::external, false) {
                action = Some(RowAction::OpenInBrowser);
            }
            if menu_item(ui, "加入播放列表", icons::queue, false) {
                action = Some(RowAction::Enqueue);
            }
            if menu_item(ui, "下一首播放", icons::next, false) {
                action = Some(RowAction::EnqueueNext);
            }
            if menu_item(ui, "缓存到本地", icons::download, false) {
                action = Some(RowAction::Cache);
            }
            if menu_item(ui, "导出为 m4a", icons::folder, false) {
                action = Some(RowAction::Export);
            }
        });
    if current {
        widgets::paint_badge(ui, badge_rect, &Badge::quality(app.quality));
    }

    let text_rect = Rect::from_min_max(
        Pos2::new(cover_rect.right() + 14.0, rect.top() + 6.0),
        Pos2::new(badge_rect.left() - 14.0, rect.bottom() - 6.0),
    );
    let title_font = t::ui_font(14.0);
    let title_color = if current { accent.ink } else { t::FG };
    let title = widgets::elided(ui, &track.title, title_font, title_color, text_rect.width());
    let title_y = if compact {
        text_rect.center().y - 3.0
    } else {
        text_rect.top() + 11.0
    };
    widgets::paint_galley_centred(
        &painter,
        Pos2::new(text_rect.left(), title_y),
        title,
        title_color,
    );

    // Sub-line: the uploader, then the resolved quality (or the video id, so
    // the line carries real information for tracks we have not played yet).
    let sub_font = t::ui_font(11.5);
    let sub_y = title_y + if compact { 13.0 } else { 17.0 };
    let author = if track.author.is_empty() {
        "未知作者"
    } else {
        track.author.as_str()
    };
    let author_width = widgets::measure(ui, author, sub_font.clone()).x;
    widgets::paint_label(
        &painter,
        Pos2::new(text_rect.left(), sub_y),
        Align2::LEFT_CENTER,
        author,
        sub_font.clone(),
        t::FG_2,
    );
    let mut x = text_rect.left() + author_width + 7.0;
    painter.circle_filled(Pos2::new(x + 1.5, sub_y), 1.5, t::FG_4);
    x += 10.0;

    if current {
        widgets::paint_label(
            &painter,
            Pos2::new(x, sub_y),
            Align2::LEFT_CENTER,
            app.quality
                .map(|quality| quality.label())
                .unwrap_or("解析中"),
            sub_font,
            if app.quality == Some(crate::api::models::AudioQuality::Flac) {
                t::GOLD
            } else {
                t::FG_3
            },
        );
    } else {
        widgets::paint_label(
            &painter,
            Pos2::new(x, sub_y),
            Align2::LEFT_CENTER,
            &track.bvid,
            t::mono_font(10.5),
            t::FG_4,
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if action.is_some() {
        return action;
    }
    response.clicked().then_some(RowAction::Play)
}

// ---------------------------------------------------------------------------
// status bar
// ---------------------------------------------------------------------------

impl App {
    pub(crate) fn ui_status_bar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        hline(ui, rect.top() + 0.5, t::LINE);

        let expired = self
            .status
            .as_ref()
            .is_some_and(|status| status.at.elapsed() > super::app::STATUS_TTL);
        if expired {
            self.status = None;
        }

        ui.horizontal_centered(|ui| {
            let status = self.status.as_ref().map(|s| (s.text.clone(), s.is_error));
            match status {
                Some((text, is_error)) => {
                    status_dot(ui, if is_error { t::ERR } else { t::OK });
                    status_text(
                        ui,
                        &text,
                        t::ui_font(11.5),
                        if is_error { t::ERR } else { t::FG_2 },
                    );
                }
                None => {
                    let logged_in = self.user.is_some();
                    status_dot(ui, if logged_in { t::OK } else { t::FG_4 });
                    let hint = if logged_in {
                        "已登录 · 无损优先可用"
                    } else {
                        "未登录（可正常听歌；登录后可获取更高音质与「我的」内容）"
                    };
                    status_text(ui, hint, t::ui_font(11.5), t::FG_3);
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Right-to-left, so the first thing added ends up rightmost:
                // `配置：/path [copy][open]`. The config keeps the spot it had.
                //
                // Both paths share one budget, derived from what the row has
                // left after the hint. Without it a long path runs into the hint
                // (measured: the cache label landed 100px inside the logged-out
                // hint) and a long one on a narrow window collides regardless, so
                // a path that does not fit is elided rather than overlapped.
                let room = ((ui.available_width() - 40.0) / 2.0 - 100.0).max(70.0);
                let config_file = crate::config::Config::path();
                self.status_path(
                    ui,
                    "配置：",
                    &crate::config::config_path_display(),
                    if config_file.exists() {
                        config_file
                    } else {
                        crate::platform::config_dir()
                    },
                    room,
                );

                ui.add_space(14.0);
                let cache_root = self.cache_root();
                self.status_path(
                    ui,
                    "缓存：",
                    &cache_root.display().to_string(),
                    cache_root,
                    room,
                );

                if let Some(err) = &self.engine_error {
                    ui.add_space(8.0);
                    status_text(ui, err, t::ui_font(11.0), t::ERR);
                }
            });
        });

        self.ui_toast(ui);
    }

    /// One `标签：路径` item at the right of the status bar, with a copy button
    /// and one that opens it.
    ///
    /// Both paths live here rather than only in the settings sheet, because that
    /// sheet scrolls and its path group sits below the fold in a normal window —
    /// which is how "where is the cache?" became a question at all. This bar is
    /// always on screen.
    ///
    /// The label rides the UI face rather than the path's monospace one. Within
    /// the monospace family egui places a *fallback* face with its own ascent,
    /// which measured two to three pixels above the Latin baseline — so in one
    /// mono string the Chinese label floated above the path and the row looked
    /// ragged. Zero spacing keeps the two pieces reading as the single line they
    /// are.
    fn status_path(
        &mut self,
        ui: &mut Ui,
        label: &str,
        path: &str,
        target: std::path::PathBuf,
        room: f32,
    ) {
        if widgets::icon_button(
            ui,
            icons::external,
            20.0,
            false,
            self.theme.accent,
            "在文件管理器中打开",
        )
        .clicked()
        {
            if let Err(err) = crate::platform::reveal(&target) {
                self.notify(err);
            }
        }
        if widgets::icon_button(ui, icons::copy, 20.0, false, self.theme.accent, "复制路径")
            .clicked()
        {
            ui.ctx().copy_text(path.to_owned());
            self.notify("已复制到剪贴板");
        }
        let font = t::mono_font(10.5);
        // Take only the width the path needs, so a short path does not push the
        // other one out of the row; elide when the budget is smaller than that.
        let width = widgets::measure(ui, path, &font).x.min(room);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::hover());
        widgets::clipped_line(ui, rect, path, font, t::FG_3, false);
        status_text(ui, label, t::ui_font(10.5), t::FG_3);
    }

    fn ui_toast(&mut self, ui: &mut Ui) {
        let Some((text, _)) = self.toast.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -118.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(&ctx, |ui| {
                let font = t::ui_font(12.5);
                let width = widgets::measure(ui, &text, &font).x + 46.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::hover());
                let painter = ui.painter().clone();
                painter.rect_filled(rect, 17.0, t::INK_4);
                painter.rect_stroke(
                    rect,
                    17.0,
                    Stroke::new(1.0, t::LINE_2),
                    egui::StrokeKind::Inside,
                );
                icons::check(
                    &painter,
                    Rect::from_min_size(
                        Pos2::new(rect.left() + 15.0, rect.center().y - 7.0),
                        Vec2::splat(14.0),
                    ),
                    t::OK,
                );
                widgets::paint_label(
                    &painter,
                    Pos2::new(rect.left() + 36.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &text,
                    font,
                    t::FG,
                );
            });
    }
}

/// The little status dot.
fn status_dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 2.5, color);
}

/// One status-bar label, centred on its glyph ink rather than on the font box.
///
/// A stock `Label` centres the ascent/descent box, and the CJK faces this app
/// loads reserve far more room below the baseline than their glyphs actually
/// use — so the dot and the config path beside the hint ended up a few pixels
/// lower than the Chinese text. Painting the ink instead (the same rule
/// `widgets::paint_label` follows) keeps every item on one centre line.
fn status_text(ui: &mut Ui, text: &str, font: egui::FontId, color: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    widgets::paint_galley_centred(
        ui.painter(),
        Pos2::new(rect.left(), rect.center().y),
        galley,
        color,
    );
}

#[cfg(test)]
mod tests {
    //! The search field is hand-painted, so these drive the real top bar and
    //! then inspect both the state and what egui actually painted.

    use super::*;
    use crate::config::Config;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

    fn raw(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0))),
            events,
            ..Default::default()
        }
    }

    type Render = fn(&egui::Context, &mut App, Vec<Event>) -> egui::FullOutput;

    /// The top bar, where the search field lives.
    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(raw(events), |ui| {
            egui::Panel::top("top_bar")
                .exact_size(theme::TOP_BAR_H)
                .frame(frames::top_bar())
                .show(ui, |ui| app.ui_top_bar(ui));
        });
        output.textures_delta.clear();
        output
    }

    /// The central pane, where the empty search state lives.
    fn central(ctx: &egui::Context, app: &mut App, events: Vec<Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(raw(events), |ui| {
            egui::CentralPanel::default()
                .frame(frames::list_pane())
                .show(ui, |ui| app.ui_central(ui));
        });
        output.textures_delta.clear();
        output
    }

    /// The settings sheet, where the config and cache paths live.
    ///
    /// Two things about it shape the tests below. It sits in an `egui` `Area`,
    /// and a new area paints nothing until the pass *after* it is first laid out
    /// — hence [`warm_settings`]. And it scrolls, so anything past the fold is
    /// never painted; this renders at a window tall enough for the whole sheet,
    /// otherwise assertions about its lower half would silently pass on nothing.
    fn settings(ctx: &egui::Context, app: &mut App, events: Vec<Event>) -> egui::FullOutput {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 1400.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            app.ui_settings_sheet(ui.ctx());
        });
        output.textures_delta.clear();
        output
    }

    /// Lay the settings sheet out until it stops moving.
    ///
    /// Two reasons it takes more than one pass: it lives in an `egui` `Area`,
    /// which paints nothing until the pass after it is first laid out, and each
    /// path row places its buttons from the *elided* text width — so the buttons
    /// shift sideways on the pass that first paints them. Reading a button's
    /// rect before that lands puts a test's click into empty space.
    fn warm_settings(ctx: &egui::Context, app: &mut App) {
        for _ in 0..3 {
            settings(ctx, app, vec![]);
        }
    }

    /// The row that is playing marks itself with the level meter instead of its
    /// position number: the number column is the one place in a row that could
    /// say "this is the one playing", and a number cannot say it.
    #[test]
    fn the_playing_row_marks_itself_instead_of_showing_its_number() {
        let (mut app, ctx) = app_with_results();
        let playing = app.results[0].clone();
        app.current = Some(playing);
        app.loading = false;

        central(&ctx, &mut app, vec![]);
        let output = central(&ctx, &mut app, vec![]);
        let texts: Vec<String> = painted_text(&output)
            .into_iter()
            .map(|(text, _)| text)
            .collect();

        assert!(
            texts.iter().any(|text| text == "02"),
            "the other rows keep their numbers: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == "01"),
            "the playing row must not show a number: {texts:?}"
        );

        // And a marker is painted where that number was: the pause glyph is two
        // small filled bars in the left-hand column.
        let bars = painted_rects(&output)
            .into_iter()
            .filter(|shape| {
                shape.rect.left() < 60.0
                    && shape.rect.width() <= 4.0
                    && shape.rect.height() <= 12.0
                    && shape.rect.height() >= 3.0
            })
            .count();
        assert!(
            bars >= 2,
            "expected the pause glyph's two bars in the index column, found {bars}"
        );
    }

    /// Both paths are on screen with no scrolling and no clicking.
    ///
    /// This is the guarantee the settings sheet cannot give: it scrolls, and its
    /// path group sits below the fold in an ordinary window. A test of mine once
    /// asserted the opposite about the sheet and passed only because it counted
    /// shapes the clip rect had already thrown away.
    #[test]
    fn the_status_bar_shows_both_the_config_and_cache_paths() {
        let (mut app, ctx) = focused_app(&[]);

        // The bar is a bottom panel; a full-screen `Ui` gives it the width it
        // would have. Two passes so the toast area, which is its own `Area`, has
        // settled.
        let mut output = ctx.run_ui(raw(vec![]), |ui| {
            app.ui_status_bar(ui);
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(raw(vec![]), |ui| {
            app.ui_status_bar(ui);
        });
        output.textures_delta.clear();

        let texts: Vec<String> = painted_text(&output)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        let cache = crate::platform::cache_dir().display().to_string();
        assert!(
            texts.iter().any(|text| text.starts_with(&cache)),
            "the cache path should be in the bar: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains(&crate::config::config_path_display())),
            "the config path should still be in the bar: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "缓存：") && texts.iter().any(|text| text == "配置："),
            "both labels should be there: {texts:?}"
        );

        // Two paths plus four buttons is a lot of bar. The hint on the left must
        // still be clear of them, or the row becomes one run-on string.
        let rects = painted_text(&output);
        let rect_of = |pred: &dyn Fn(&str) -> bool| {
            rects
                .iter()
                .find(|(text, _)| pred(text))
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("missing from the bar: {texts:?}"))
        };
        let hint = rect_of(&|text| text.starts_with("已登录") || text.starts_with("未登录"));
        let cache_label = rect_of(&|text| text == "缓存：");
        assert!(
            hint.right() < cache_label.left(),
            "the hint ends at {} but the cache label starts at {}",
            hint.right(),
            cache_label.left()
        );
    }

    /// The sheet has to name the cache directory: it is where the disk actually
    /// goes, and there was previously no way to find it from inside the app.
    #[test]
    fn the_settings_sheet_shows_where_the_cache_is() {
        let (mut app, ctx) = focused_app(&[]);
        app.settings_open = true;
        warm_settings(&ctx, &mut app);
        let output = settings(&ctx, &mut app, vec![]);

        let cache = crate::platform::cache_dir().display().to_string();
        let texts: Vec<String> = painted_text(&output)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert!(
            texts
                .iter()
                .any(|text| *text == cache || cache.starts_with(text.as_str())),
            "the cache path should be painted (elided if it is long): {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("缓存占用")),
            "the sheet should say how much the cache is using: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains(&crate::config::config_path_display())),
            "the config path should still be there: {texts:?}"
        );
    }

    /// The copy and reveal buttons belong to the cache row, and the copy one has
    /// to put the cache path — not the config path — on the clipboard.
    #[test]
    fn the_cache_path_can_be_copied_and_revealed() {
        let (mut app, ctx) = focused_app(&[]);
        app.settings_open = true;
        warm_settings(&ctx, &mut app);
        settings(&ctx, &mut app, vec![]);

        let copy = ctx
            .read_response(egui::Id::new("cache-0"))
            .expect("the cache row should have a copy button");
        assert!(
            ctx.read_response(egui::Id::new("cache-1")).is_some(),
            "the cache row should have a reveal button"
        );

        let released = click_at_capturing(&ctx, &mut app, copy.rect.center(), settings);
        let expected = crate::platform::cache_dir().display().to_string();
        let copied = released.platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == &expected),
        );
        assert!(
            copied,
            "缓存按钮应该把缓存路径交给剪贴板（{expected}），实际是 {:?}",
            released.platform_output.commands
        );
    }

    /// Every string egui *actually shows*, with the rect it was painted into.
    ///
    /// Clip-aware on purpose: a shape scrolled out of a `ScrollArea`, or cut by a
    /// panel, is still in the shape list. Counting it would let a test about "is
    /// this on screen" pass on something nobody can see — which is exactly how an
    /// earlier test of mine claimed the cache path was above the fold.
    fn painted_text(output: &egui::FullOutput) -> Vec<(String, Rect)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(text) => {
                    let rect = Rect::from_min_size(text.pos, text.galley.size());
                    out.push((text.galley.text().to_owned(), rect));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in &output.shapes {
            if !clipped
                .clip_rect
                .intersects(clipped.shape.visual_bounding_rect())
            {
                continue;
            }
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn painted(output: &egui::FullOutput, needle: &str) -> bool {
        painted_text(output)
            .iter()
            .any(|(text, _)| text.contains(needle))
    }

    /// The rect `label` was painted into.
    fn painted_rect(output: &egui::FullOutput, label: &str) -> Option<Rect> {
        painted_text(output)
            .into_iter()
            .find(|(text, _)| text == label)
            .map(|(_, rect)| rect)
    }

    /// An app showing a finished search of two results.
    fn app_with_results() -> (App, egui::Context) {
        let (mut app, ctx) = focused_app(&[]);
        app.search_history_open = false;
        app.searching = false;
        app.last_keyword = "周杰伦".to_owned();
        app.results = ["BV1demo0001", "BV1demo0002"]
            .iter()
            .map(|bvid| Track {
                bvid: (*bvid).to_owned(),
                source: crate::api::models::Source::Bilibili,
                aid: 0,
                cid: 0,
                title: format!("【无损音质】盘点{bvid}首经典歌曲"),
                author: "华语音乐馆".to_owned(),
                duration: 271,
                cover: None,
            })
            .collect();
        (app, ctx)
    }

    /// An app whose search field is focused, with `history` as its search log.
    fn focused_app(history: &[&str]) -> (App, egui::Context) {
        std::env::set_var("HOME", "/tmp/listenbli-search-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-search-tests");
        let mut app = App::new_for_tests(Config::default());
        let history: Vec<String> = history.iter().map(|entry| (*entry).to_owned()).collect();
        app.update_config(|config| config.search_history = history);
        app.search_input.clear();
        app.focus_search = true;

        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);
        // One frame to hand the field focus, one to draw the dropdown.
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![]);
        }
        (app, ctx)
    }

    /// Clicks whatever `label` was painted at.
    fn click_painted(ctx: &egui::Context, app: &mut App, label: &str, render: Render) {
        let listed = render(ctx, app, vec![]);
        let target =
            painted_rect(&listed, label).unwrap_or_else(|| panic!("{label} should be on screen"));
        let pos = target.center();
        let press = vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ];
        let release = vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ];
        render(ctx, app, vec![Event::PointerMoved(pos)]);
        render(ctx, app, press);
        render(ctx, app, release);
        render(ctx, app, vec![Event::PointerMoved(pos)]);
    }

    /// Like [`click_painted`], but returns the frame the release landed in, so a
    /// test can read what the app asked the platform to do (clipboard, opener).
    fn click_painted_capturing(
        ctx: &egui::Context,
        app: &mut App,
        label: &str,
        render: Render,
    ) -> egui::FullOutput {
        let listed = render(ctx, app, vec![]);
        let target =
            painted_rect(&listed, label).unwrap_or_else(|| panic!("{label} should be on screen"));
        click_at_capturing(ctx, app, target.center(), render)
    }

    /// Clicks an absolute position and returns the output of the release frame.
    fn click_at_capturing(
        ctx: &egui::Context,
        app: &mut App,
        pos: Pos2,
        render: Render,
    ) -> egui::FullOutput {
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        render(ctx, app, vec![Event::PointerMoved(pos)]);
        render(ctx, app, vec![Event::PointerMoved(pos), button(true)]);
        let release = render(ctx, app, vec![Event::PointerMoved(pos), button(false)]);
        render(ctx, app, vec![Event::PointerMoved(pos)]);
        release
    }

    /// Clicks an absolute position: for hitting empty space, where nothing is
    /// painted to aim at.
    fn click_at(ctx: &egui::Context, app: &mut App, pos: Pos2, render: Render) {
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        render(ctx, app, vec![Event::PointerMoved(pos)]);
        render(ctx, app, vec![Event::PointerMoved(pos), button(true)]);
        render(ctx, app, vec![Event::PointerMoved(pos), button(false)]);
        render(ctx, app, vec![Event::PointerMoved(pos)]);
    }

    /// Running a search writes it to the persisted history.
    #[test]
    fn a_search_is_remembered() {
        let (mut app, _ctx) = focused_app(&[]);
        app.search_input = "周杰伦 晴天".to_owned();
        app.start_search();
        assert_eq!(app.search_history(), ["周杰伦 晴天"]);

        app.search_input = "  ".to_owned();
        app.start_search();
        assert_eq!(app.search_history(), ["周杰伦 晴天"]);
    }

    /// An empty, focused field offers the recent searches.
    #[test]
    fn the_search_field_offers_the_recent_searches() {
        let (_app, ctx) = focused_app(&["米津玄師", "晴天"]);
        let mut app = _app;

        let output = frame(&ctx, &mut app, vec![]);
        assert!(
            painted(&output, "搜索记录"),
            "the dropdown should be titled"
        );
        assert!(
            painted(&output, "米津玄師"),
            "the newest entry should be listed"
        );
        assert!(
            painted(&output, "晴天"),
            "older entries should be listed too"
        );
        assert!(
            painted(&output, "清空搜索记录"),
            "the history should be clearable"
        );

        // Typing narrows the field to a search, so the history steps aside.
        app.search_input = "晴".to_owned();
        let output = frame(&ctx, &mut app, vec![]);
        assert!(!painted(&output, "清空搜索记录"));
    }

    /// The mouse release that focuses the field must not dismiss the list that
    /// same click just opened: egui forgives clicks landing on the popup, but
    /// the release over the field counts as "clicked outside" and closed it the
    /// frame after it appeared. ⌘K never showed it because it is not a click.
    #[test]
    fn clicking_the_search_field_keeps_the_history_up() {
        let (mut app, ctx) = focused_app(&["米津玄師", "晴天"]);
        // Back to the state before the user touches the field.
        app.search_history_open = false;
        app.focus_search = false;
        ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::new("search-field")));
        let output = frame(&ctx, &mut app, vec![]);
        assert!(!painted(&output, "搜索记录"), "the list starts closed");

        click_painted(&ctx, &mut app, "搜索歌曲 / 视频，如：周杰伦 晴天", frame);

        let output = frame(&ctx, &mut app, vec![]);
        assert!(
            painted(&output, "搜索记录"),
            "the field's own click should leave the list up"
        );

        // A click on empty space, away from field and list, still dismisses it.
        click_at(&ctx, &mut app, Pos2::new(900.0, 760.0), frame);
        let output = frame(&ctx, &mut app, vec![]);
        assert!(
            !painted(&output, "搜索记录"),
            "clicking away should still close the list"
        );
    }

    /// Clicking an entry re-runs that search and promotes it to the top.
    #[test]
    fn clicking_a_recent_search_runs_it_again() {
        let (mut app, ctx) = focused_app(&["米津玄師", "晴天"]);

        click_painted(&ctx, &mut app, "晴天", frame);

        assert_eq!(
            app.search_input, "晴天",
            "the entry should re-run as a search"
        );
        // Running it moved it back to the top of the list.
        assert_eq!(app.search_history()[0], "晴天");
    }

    /// The empty search pane offers the same log as chips.
    #[test]
    fn the_empty_pane_lists_the_recent_searches() {
        let (mut app, ctx) = focused_app(&["米津玄師", "晴天"]);
        app.search_history_open = false;

        let output = central(&ctx, &mut app, vec![]);
        assert!(
            painted(&output, "最近的搜索记录"),
            "the pane should explain the chips"
        );
        assert!(painted(&output, "米津玄師") && painted(&output, "晴天"));

        click_painted(&ctx, &mut app, "晴天", central);
        assert_eq!(app.search_input, "晴天", "a chip should re-run that search");
        assert_eq!(app.search_history()[0], "晴天");
    }

    /// Every rectangle egui painted, so a test can check the shape of the
    /// hand-painted search pill instead of trusting the layout code.
    fn painted_rects(output: &egui::FullOutput) -> Vec<egui::epaint::RectShape> {
        fn walk(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
            match shape {
                egui::Shape::Rect(rect) => out.push(rect.clone()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn search_pill(output: &egui::FullOutput) -> egui::epaint::RectShape {
        painted_rects(output)
            .into_iter()
            .find(|rect| rect.corner_radius.nw == 18 && rect.rect.width() > 300.0)
            .expect("the search pill should be painted")
    }

    /// The search field is a 36px pill centred in the top bar.
    ///
    /// It used to stretch to the whole bar: a single `set_min_height` let the
    /// text edit's growing atom expand the frame, so the "pill" became a slab
    /// and the focus ring hugged the bar's own edges.
    #[test]
    fn the_search_pill_keeps_its_height() {
        let (mut app, ctx) = focused_app(&[]);
        let output = frame(&ctx, &mut app, vec![]);

        let pill = search_pill(&output);
        assert!(
            (34.0..=40.0).contains(&pill.rect.height()),
            "the pill should stay ~36px tall, got {:?}",
            pill.rect
        );
    }

    /// The focus halo is flush against the pill and shares its radius: a gap
    /// (or a different radius) reads as a broken, doubled outline.
    #[test]
    fn the_focus_halo_hugs_the_pill() {
        let (mut app, ctx) = focused_app(&[]);
        app.focus_search = true;
        let accent = app.theme.accent;
        let output = frame(&ctx, &mut app, vec![]);

        let pill = search_pill(&output).rect;
        let halo = painted_rects(&output)
            .into_iter()
            .find(|rect| rect.stroke.color == accent.dim() && rect.stroke.width > 0.0)
            .expect("a focused field should paint an accent halo");

        assert_eq!(halo.rect, pill, "the halo must sit on the pill itself");
        assert_eq!(halo.corner_radius.nw, 18, "and share the pill's radius");
    }

    /// Control case: a click on a row *inside a scroll area* must register.
    #[test]
    fn clicking_a_result_plays_it() {
        let (mut app, ctx) = app_with_results();
        click_painted(&ctx, &mut app, "BV1demo0001", central);
        assert_eq!(
            app.current.as_ref().map(|t| t.bvid.as_str()),
            Some("BV1demo0001"),
            "点击结果行应开始播放"
        );
    }

    /// Where a row's `⋯` sits: the button is 12+42px clear of the duration
    /// column and its own centre is 8px in, so it is 41px right of the centred
    /// duration text. Aiming from painted geometry keeps the test honest — if
    /// the row layout moves the button, the click misses and this fails.
    fn more_button_of(painted: &[(String, Rect)], row: usize) -> Pos2 {
        let durations: Vec<Rect> = painted
            .iter()
            .filter(|(text, _)| text == "04:31")
            .map(|(_, rect)| *rect)
            .collect();
        let rect = durations
            .get(row)
            .unwrap_or_else(|| panic!("row {row} should paint its duration: {durations:?}"));
        Pos2::new(rect.center().x + 41.0, rect.center().y)
    }

    /// Clicking `⋯` must open the row menu, not start the track: the button owns
    /// its own rect, registered after the row's.
    #[test]
    fn the_row_menu_button_does_not_play_the_track() {
        let (mut app, ctx) = app_with_results();
        let listed = painted_text(&central(&ctx, &mut app, vec![]));
        let target = more_button_of(&listed, 0);

        click_at(&ctx, &mut app, target, central);

        assert!(
            app.current.is_none(),
            "点 ⋯ 不应该开始播放，当前却变成了 {:?}",
            app.current.as_ref().map(|t| t.bvid.as_str())
        );
        let output = central(&ctx, &mut app, vec![]);
        assert!(
            painted(&output, "复制视频链接"),
            "菜单应该弹出来：{:?}",
            painted_text(&output)
                .into_iter()
                .map(|(text, _)| text)
                .collect::<Vec<_>>()
        );
    }

    /// Every entry the menu promises is really there.
    #[test]
    fn the_row_menu_lists_its_actions() {
        let (mut app, ctx) = app_with_results();
        let listed = painted_text(&central(&ctx, &mut app, vec![]));
        let target = more_button_of(&listed, 0);
        click_at(&ctx, &mut app, target, central);

        let output = central(&ctx, &mut app, vec![]);
        for label in [
            "复制视频链接",
            "在浏览器打开",
            "加入播放列表",
            "下一首播放",
            "缓存到本地",
        ] {
            assert!(painted(&output, label), "菜单里应该有 {label}");
        }
    }

    /// 复制视频链接 hands the bilibili page URL to the clipboard.
    #[test]
    fn copying_a_video_link_reaches_the_clipboard() {
        let (mut app, ctx) = app_with_results();
        let listed = painted_text(&central(&ctx, &mut app, vec![]));
        let target = more_button_of(&listed, 0);
        click_at(&ctx, &mut app, target, central);

        let output = click_painted_capturing(&ctx, &mut app, "复制视频链接", central);

        let wanted = util::video_url("BV1demo0001");
        let copied = output.platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == &wanted),
        );
        assert!(
            copied,
            "应该把 {wanted} 交给剪贴板，实际是 {:?}",
            output.platform_output.commands
        );
    }

    /// The two queue entries differ: one appends, one goes right after the
    /// current track.
    #[test]
    fn the_row_menu_queues_append_or_jump_the_line() {
        let (mut app, ctx) = app_with_results();
        let listed = painted_text(&central(&ctx, &mut app, vec![]));
        let target = more_button_of(&listed, 0);
        click_at(&ctx, &mut app, target, central);
        click_painted(&ctx, &mut app, "加入播放列表", central);

        let queued: Vec<&str> = app.queue.iter().map(|t| t.bvid.as_str()).collect();
        assert_eq!(queued, ["BV1demo0001"], "加入播放列表应该把它排到队尾");

        // The same row again: a duplicate is refused instead of queued twice.
        let listed = painted_text(&central(&ctx, &mut app, vec![]));
        let target = more_button_of(&listed, 0);
        click_at(&ctx, &mut app, target, central);
        click_painted(&ctx, &mut app, "加入播放列表", central);
        assert_eq!(app.queue.len(), 1, "同一首不应该被排队两次");

        // 下一首播放 on the *second* row inserts right after the first.
        let listed = painted_text(&central(&ctx, &mut app, vec![]));
        let target = more_button_of(&listed, 1);
        click_at(&ctx, &mut app, target, central);
        click_painted(&ctx, &mut app, "下一首播放", central);

        let queued: Vec<&str> = app.queue.iter().map(|t| t.bvid.as_str()).collect();
        assert_eq!(queued, ["BV1demo0001", "BV1demo0002"]);
    }

    /// Paging has no button any more: the tail of the list asks for the next
    /// page on its own, and the reply that says "nothing more" stops it.
    #[test]
    fn the_end_of_the_list_fetches_the_next_page() {
        let (mut app, ctx) = app_with_results();
        app.search_has_more = true;
        app.search_page = 1;

        // Two rows cannot fill the viewport, so the list is already at its end
        // and the first frame asks for page 2.
        let output = central(&ctx, &mut app, vec![]);
        assert!(
            app.loading_more,
            "the last rows should pull in the next page on their own"
        );
        assert!(
            !painted(&output, "加载更多"),
            "the manual button should be gone"
        );

        // The last page reports nothing more, so nothing is asked for.
        app.search_has_more = false;
        app.loading_more = false;
        central(&ctx, &mut app, vec![]);
        assert!(!app.loading_more, "the last page must not ask for more");
    }

    /// The tail-fetch trigger: only the last rows (or a list too short to
    /// scroll) pull in the next page.
    #[test]
    fn the_next_page_is_only_fetched_near_the_end() {
        // Mid-list, more than the margin above the end: wait for the scroll.
        assert!(!near_end(0.0, 600.0, 4000.0));
        assert!(!near_end(3000.0, 600.0, 4000.0));
        // Inside the margin, and right at the end.
        assert!(near_end(3081.0, 600.0, 4000.0));
        assert!(near_end(3400.0, 600.0, 4000.0));
        // Shorter than its viewport: there is nothing to scroll, so it counts
        // as the end or it could never grow.
        assert!(near_end(0.0, 600.0, 120.0));
    }

    /// The dropdown can be emptied, and closes itself once it is.
    #[test]
    fn the_recent_searches_can_be_cleared() {
        let (mut app, ctx) = focused_app(&["米津玄師", "晴天"]);
        assert!(app.search_history_open);

        click_painted(&ctx, &mut app, "清空搜索记录", frame);

        assert!(app.search_history().is_empty(), "the log should be cleared");
        assert!(
            !app.search_history_open,
            "an empty history should dismiss the dropdown"
        );
    }

    /// The search pill is its own 318x36 box: it must not swallow the right
    /// cluster. An unbounded pill let its right-to-left run lay the ⌘K chip
    /// against the end of the top bar, which dragged the frame's fill — and the
    /// `focus-within` ring — around 无损优先 / 设置 / 账号, and hid the chip
    /// under the account chip.
    #[test]
    fn the_search_pill_does_not_swallow_the_top_right_cluster() {
        let (mut app, ctx) = focused_app(&[]);
        let output = frame(&ctx, &mut app, vec![]);

        fn rounded(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
            match shape {
                egui::Shape::Rect(rect) => out.push(rect.clone()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        rounded(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut all = Vec::new();
        for clipped in &output.shapes {
            rounded(&clipped.shape, &mut all);
        }
        // The pill: the 18px-radius rounded rect carrying the field's fill.
        let pill = all
            .iter()
            .find(|rect| rect.corner_radius.nw == 18 && rect.fill != Color32::TRANSPARENT)
            .map(|rect| rect.rect)
            .expect("the search pill should be painted");

        let chip = painted_rect(&output, "⌘K").expect("the ⌘K chip should be painted");
        assert!(
            pill.contains(chip.center()),
            "the ⌘K chip at {chip:?} should sit inside the pill {pill:?}"
        );
        let cluster = painted_rect(&output, "无损优先").expect("the lossless chip is painted");
        assert!(
            pill.right() < cluster.left(),
            "the pill {pill:?} should end before the right cluster at {cluster:?}"
        );
    }

    /// The status bar's dot, hint and config path must share one centre line,
    /// and the two halves of the config line one baseline.
    ///
    /// A stock `Label` centres the font's ascent/descent box, and the CJK faces
    /// this app loads leave a few pixels of unused descent under the glyphs —
    /// which left the Chinese hint visibly high next to the dot and the path.
    /// The bar paints its text ink-centred instead; this keeps it that way, and
    /// separately checks the `配置：` label against the path beside it, which is
    /// the pair that shared a monospace family and drifted apart.
    #[test]
    fn the_status_bar_centres_every_item_on_one_line() {
        std::env::set_var("HOME", "/tmp/listenbli-search-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-search-tests");
        let mut app = App::new_for_tests(Config::default());
        let ctx = egui::Context::default();
        // The same face the running app resolves, so the CJK metrics are real.
        let cjk = crate::config::with(&app.config, |c| {
            crate::platform::resolve_cjk_font(&c.cjk_font, c.cjk_font_path.as_deref())
        });
        crate::ui::theme::install_fonts(&ctx, cjk.as_deref());
        crate::ui::theme::install_style(&ctx, &app.theme);

        let mut output = ctx.run_ui(raw(vec![]), |ui| {
            egui::Panel::bottom("status_bar")
                .exact_size(theme::STATUS_BAR_H)
                .frame(frames::status_bar())
                .show(ui, |ui| app.ui_status_bar(ui));
        });
        output.textures_delta.clear();

        /// One painted label: where it was drawn, and the baselines of the
        /// glyphs this test looks at (relative to the shape's origin).
        struct Painted {
            text: String,
            pos: Pos2,
            ink_center: f32,
            baselines: Vec<(char, f32)>,
        }

        fn walk(shape: &egui::Shape, ink: &mut Vec<Painted>, dot: &mut Option<f32>) {
            match shape {
                egui::Shape::Text(text) => {
                    let mut baselines = Vec::new();
                    for row in &text.galley.rows {
                        for glyph in &row.row.glyphs {
                            baselines.push((glyph.chr, row.pos.y + glyph.pos.y));
                        }
                    }
                    ink.push(Painted {
                        text: text.galley.text().to_owned(),
                        pos: text.pos,
                        ink_center: text.galley.mesh_bounds.center().y,
                        baselines,
                    });
                }
                egui::Shape::Circle(circle) => *dot = Some(circle.center.y),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, ink, dot);
                    }
                }
                _ => {}
            }
        }
        let (mut ink, mut dot) = (Vec::new(), None);
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut ink, &mut dot);
        }

        let dot = dot.expect("the status dot should be painted");
        assert!(
            ink.len() >= 3,
            "the bar should paint its hint, label and path: {:?}",
            ink.iter().map(|item| &item.text).collect::<Vec<_>>()
        );
        for item in &ink {
            let centre = item.pos.y + item.ink_center;
            assert!(
                (centre - dot).abs() < 0.5,
                "{:?} is centred at {centre}, but the dot is at {dot}",
                item.text
            );
        }

        // `配置：` rides the UI face and the path the monospace one; the label
        // must not float above the path the way it did inside a single mono
        // string, so compare the two baselines. The deepest glyph of a row is
        // the baseline of its tallest face, which is the line the Latin sits on
        // — and unlike looking up `配` by character this works on a runner with
        // no CJK font at all, where the glyph is a replacement.
        let baseline = |item: &Painted| {
            let deepest = item
                .baselines
                .iter()
                .map(|(_, y)| *y)
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                deepest.is_finite(),
                "{:?} should have been laid out with glyphs",
                item.text
            );
            item.pos.y + deepest
        };
        let label = ink
            .iter()
            .find(|item| item.text.starts_with("配置"))
            .expect("the config label should be painted on its own");
        let path = ink
            .iter()
            .find(|item| item.text.ends_with("config.json"))
            .expect("the config path should be painted");
        let (label_base, path_base) = (baseline(label), baseline(path));
        assert!(
            (label_base - path_base).abs() <= 1.5,
            "配置 sits on baseline {label_base} but the path on {path_base}"
        );
    }
}

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

        let frame = egui::Frame::NONE
            .fill(Color32::from_black_alpha(107))
            .stroke(Stroke::new(1.0, t::LINE))
            .corner_radius(18.0)
            .inner_margin(Margin {
                left: 12,
                right: 10,
                top: 0,
                bottom: 0,
            });

        let inner = frame.show(ui, |ui| {
            ui.set_min_width(318.0);
            ui.set_min_height(36.0);

            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(15.0, 36.0), Sense::hover());
            icons::search(
                ui.painter(),
                Rect::from_center_size(icon_rect.center(), Vec2::splat(15.0)),
                t::FG_4,
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

        let focused = inner.inner.has_focus();
        if focused {
            let painter = ui.painter().clone();
            painter.rect_stroke(
                inner.response.rect,
                18.0,
                Stroke::new(1.0, t::fade(accent.accent, 0.55)),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                inner.response.rect.expand(3.0),
                21.0,
                Stroke::new(3.0, accent.dim()),
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

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                column_head(ui, 24.0);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        for (index, track) in tracks.iter().enumerate() {
                            if track_row(ui, self, track, index).clicked() {
                                clicked = Some(index);
                            }
                            ui.add_space(1.0);
                        }
                    });
                });
                ui.add_space(18.0);
                if has_more || self.loading_more {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        if self.loading_more {
                            spinner(ui, 15.0, accent);
                        } else if ghost_button(
                            ui,
                            "加载更多",
                            Some(icons::chevron),
                            accent,
                            false,
                            true,
                        )
                        .clicked()
                        {
                            load_more = true;
                        }
                    });
                }
                ui.add_space(22.0);
            });

        match source {
            PaneSource::Search => self.results = tracks,
            PaneSource::Favorites => self.fav_items = tracks,
            PaneSource::History => self.history = tracks,
        }

        if let Some(index) = clicked {
            let list = match source {
                PaneSource::Search => self.results.clone(),
                PaneSource::Favorites => self.fav_items.clone(),
                PaneSource::History => self.history.clone(),
            };
            self.play_from_list(&list, index);
        }
        if load_more {
            match source {
                // A later page of the query already on screen.
                PaneSource::Search => self.send(crate::net::Cmd::Search {
                    keyword: self.last_keyword.clone(),
                    page: self.search_page + 1,
                }),
                PaneSource::Favorites => {
                    if let Some(media_id) = self.selected_folder {
                        let page = self.fav_page + 1;
                        self.send(crate::net::Cmd::LoadFavItems { media_id, page });
                    }
                }
                PaneSource::History => {}
            }
        }
    }
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

/// One row of the list. Clicking anywhere on it plays that track.
pub(crate) fn track_row(ui: &mut Ui, app: &App, track: &Track, index: usize) -> egui::Response {
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
    widgets::paint_label(
        &painter,
        idx_rect.center(),
        Align2::CENTER_CENTER,
        format!("{:02}", index + 1),
        t::mono_font(11.5),
        t::FG_3,
    );

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
    if hovered {
        painter.rect_filled(more_rect.expand(4.0), t::R_SM, t::white(0.07));
        icons::more(&painter, more_rect, t::FG);
    }
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
    response
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
                    ui.label(RichText::new(text).size(11.5).color(if is_error {
                        t::ERR
                    } else {
                        t::FG_2
                    }));
                }
                None => {
                    let logged_in = self.user.is_some();
                    status_dot(ui, if logged_in { t::OK } else { t::FG_4 });
                    let hint = if logged_in {
                        "已登录 · 无损优先可用"
                    } else {
                        "未登录（可正常听歌；登录后可获取更高音质与「我的」内容）"
                    };
                    ui.label(RichText::new(hint).size(11.5).color(t::FG_3));
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(
                    ui,
                    icons::copy,
                    20.0,
                    false,
                    self.theme.accent,
                    "复制配置路径",
                )
                .clicked()
                {
                    ui.ctx().copy_text(crate::config::config_path_display());
                    self.notify("已复制到剪贴板");
                }
                ui.label(
                    RichText::new(format!("配置：{}", crate::config::config_path_display()))
                        .font(t::mono_font(10.5))
                        .color(t::FG_3),
                );
                if let Some(err) = &self.engine_error {
                    ui.add_space(8.0);
                    ui.label(RichText::new(err).size(11.0).color(t::ERR));
                }
            });
        });

        self.ui_toast(ui);
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

    /// Every string egui painted, with the rect it was painted into.
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
        let mut app = App::new(Config::default());
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

    /// The results list only offers "加载更多" while the server says so, and
    /// swaps it for a spinner while the next page is in flight.
    #[test]
    fn the_results_list_offers_loading_more_pages() {
        let (mut app, ctx) = app_with_results();
        app.search_has_more = true;
        let output = central(&ctx, &mut app, vec![]);
        assert!(painted(&output, "加载更多"), "more pages should be offered");

        app.loading_more = true;
        let output = central(&ctx, &mut app, vec![]);
        assert!(
            !painted(&output, "加载更多"),
            "a page in flight replaces the button with a spinner"
        );

        app.loading_more = false;
        app.search_has_more = false;
        let output = central(&ctx, &mut app, vec![]);
        assert!(
            !painted(&output, "加载更多"),
            "the last page should not offer more"
        );
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
}

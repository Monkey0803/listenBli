//! Shared UI: the shell (top bar, list pane, status line), the reusable track
//! row, and the small buttons the panes share.

pub mod connect;
pub mod icons;
pub mod login;
pub mod lyrics;
pub mod nav;
pub mod pages;
pub mod player;
pub mod settings;
pub mod songs;
pub mod source;
pub mod theme;
pub mod widgets;

use egui::{
    Align, Align2, Color32, CursorIcon, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    TextEdit, Ui, Vec2,
};

use self::icons::Icon;
use self::theme::{self as t, Accent};
use self::widgets::chip;
use super::app::{App, Page, Tab};

/// Panel backgrounds, so the shell and the panels agree on one surface.
pub mod frames {
    use super::theme;
    use egui::{CornerRadius, Frame, Margin};

    pub fn top_bar() -> Frame {
        Frame::NONE
            .fill(theme::INK_1)
            .inner_margin(Margin::symmetric(16, 0))
    }

    /// The transport deck: a plate a shade lighter than the page above it, with a
    /// brighter rule on its top edge so the deck reads as the one thing that never
    /// changes as pages move behind it.
    pub fn deck(_theme: &theme::Theme) -> Frame {
        Frame::NONE
            .fill(theme::INK_2)
            .inner_margin(Margin::symmetric(16, 0))
    }

    /// The library rail. It owns its own background because the nav items run the
    /// full width of the plate, so a margin here would leave an unpainted strip.
    pub fn library_nav() -> Frame {
        Frame::NONE
            .fill(theme::INK_1)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::ZERO)
    }

    /// The now-playing column: the same plate as the rail, ruled off on its inner
    /// edge.
    pub fn now_playing() -> Frame {
        Frame::NONE
            .fill(theme::INK_1)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::ZERO)
    }

    /// Legacy name for the deck. `lyrics.rs` and `player.rs` still build their own
    /// test frames with it; both panels are being replaced, and the alias goes with
    /// them.
    pub fn player_bar(theme: &theme::Theme) -> Frame {
        deck(theme)
    }

    /// Legacy name for the now-playing column, kept for the same reason.
    pub fn lyrics(theme: &theme::Theme) -> Frame {
        let _ = theme;
        now_playing()
    }

    pub fn list_pane() -> Frame {
        Frame::NONE.fill(theme::INK_0).inner_margin(Margin::ZERO)
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

// ---------------------------------------------------------------------------
// top bar
// ---------------------------------------------------------------------------

impl App {
    pub(crate) fn ui_top_bar(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let playing = self.is_playing();
        let rect = ui.max_rect();
        hline(ui, rect.bottom() - 0.5, t::LINE);

        // The console's top bar answers two questions: where am I, and what is in
        // the library. The old bar answered "which platform am I browsing" — the
        // question this redesign removes — so the source switch, the account tabs
        // and the 无损优先 chip all went with it. Preferring lossless is a setting,
        // not something to keep a chip on screen for.
        ui.horizontal_centered(|ui| {
            self.brand(ui, accent, playing);
            ui.add_space(9.0);
            self.page_tag(ui, accent);
            ui.add_space(14.0);
            self.search_field(ui);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.account_control(ui, accent);
                ui.add_space(4.0);
                if widgets::icon_button(
                    ui,
                    icons::settings,
                    32.0,
                    self.settings_open,
                    accent,
                    "设置",
                )
                .clicked()
                {
                    self.settings_open = !self.settings_open;
                    self.account_menu_open = false;
                }
                ui.add_space(4.0);

                // Offline and sync are readings first and shortcuts second: both
                // open the page that explains the number.
                let offline = self.offline_chip_label();
                if chip(
                    ui,
                    "已下载",
                    Some(icons::download),
                    offline.as_deref(),
                    false,
                    accent,
                )
                .clicked()
                {
                    self.set_page(Page::Offline);
                }
                ui.add_space(4.0);

                let sync = self.sync_label().unwrap_or_else(|| "未同步".to_owned());
                if chip(ui, "同步", Some(icons::refresh), Some(&sync), false, accent).clicked() {
                    self.set_page(Page::Sources);
                }
            });
        });
    }

    /// The page you are on, as a small bordered tag beside the wordmark — the
    /// console's substitute for a title bar, since the rail already owns the
    /// navigation and a second title would be redundant.
    fn page_tag(&self, ui: &mut Ui, accent: Accent) {
        let tag = self.page.tag();
        let label = self.page.label();
        let tag_w = widgets::engraved_measure(ui, tag, 9.5).x;
        let label_font = t::ui_font(9.5);
        let label_w = widgets::measure(ui, label, &label_font).x;
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(tag_w + label_w + 21.0, 22.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_stroke(
            rect,
            t::R_SM,
            Stroke::new(1.0, t::LINE_2),
            egui::StrokeKind::Inside,
        );
        widgets::engraved_at(
            ui,
            Pos2::new(rect.left() + 7.0, rect.center().y),
            Align2::LEFT_CENTER,
            tag,
            9.5,
            accent.accent,
        );
        widgets::label_at(
            ui,
            Pos2::new(rect.left() + 7.0 + tag_w + 7.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            label_font,
            t::FG_3,
        );
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
            .corner_radius(t::R_SM)
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
        self.send(crate::net::Cmd::Search {
            source: self.source,
            keyword,
            page: 1,
        });
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
        // The library's six pages. 搜索 still runs the old results pane while it
        // is moved over to songs; the other five are the library's own.
        match self.page {
            Page::Library => self.ui_library_page(ui),
            Page::Playlists => self.ui_playlists_page(ui),
            Page::Recent => self.ui_recent_page(ui),
            Page::Offline => self.ui_offline_page(ui),
            Page::Search => self.ui_search_page(ui),
            Page::Sources => self.ui_sources_page(ui),
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
pub(crate) fn near_end(offset: f32, viewport: f32, content: f32) -> bool {
    content - (offset + viewport) <= AUTO_LOAD_MARGIN
}

// ---------------------------------------------------------------------------
// status bar
// ---------------------------------------------------------------------------

impl App {
    pub(crate) fn ui_toast(&mut self, ui: &mut Ui) {
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
    use crate::api::models::Track;

    fn app_with_results() -> (App, egui::Context) {
        let (mut app, ctx) = focused_app(&[]);
        // These are the search pane's tests, and the central pane now opens on
        // 我的歌, so they have to say which page they mean.
        app.set_page(Page::Search);
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
                album: None,
                artist: None,
            })
            .collect();
        (app, ctx)
    }

    /// An app whose search field is focused, with `history` as its search log.
    fn focused_app(history: &[&str]) -> (App, egui::Context) {
        crate::platform::serialise_home_tests();
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
        app.set_page(Page::Search);
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
        // The field is a 2px console box now, so its radius no longer identifies
        // it — assert on what the field actually contains and on where the right
        // cluster starts, which is the overlap this test exists to catch.
        let _ = &all;
        let chip = painted_rect(&output, "⌘K").expect("the ⌘K chip should be painted");
        let cluster = painted_rect(&output, "同步").expect("the sync chip is painted");
        assert!(
            chip.right() < cluster.left(),
            "the search field (ending at {chip:?}) should end before the right cluster at {cluster:?}"
        );
    }
}

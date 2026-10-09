//! The logged-in tabs: favourites and watch history.

use egui::{Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use super::icons;
use super::theme::{self as t};
use super::widgets;
use super::{ghost_button, list_header, primary_button, PaneSource};
use crate::app::App;

impl App {
    pub(crate) fn ui_favorites(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;

        if self.user.is_none() {
            list_header(ui, "收藏夹", "", |_ui| {});
            widgets::empty_state(
                ui,
                icons::lock,
                "登录后可查看收藏夹",
                "listenBli 未登录也能正常听歌，但要读取「我的」内容需要一次扫码。凭据会加密保存在本机，重启后依然有效。",
                &[],
                accent,
            );
            // The primary call to action sits under the description.
            ui.vertical_centered(|ui| {
                if primary_button(ui, "扫码登录", Some(icons::qr), accent).clicked() {
                    self.open_login();
                }
            });
            return;
        }

        let loaded = self.fav_items.len();
        let meta = if loaded > 0 {
            format!("{loaded} 首已载入")
        } else {
            "正在载入".to_owned()
        };
        let mut refresh = false;
        list_header(ui, "收藏夹", &meta, |ui| {
            if ghost_button(ui, "刷新", Some(icons::refresh), accent, false, true).clicked() {
                refresh = true;
            }
        });
        if refresh {
            if let Some(user) = &self.user {
                self.send(crate::net::Cmd::LoadFavFolders { mid: user.mid });
            }
        }

        self.folder_bar(ui, accent);

        if !self.fav_loaded && self.fav_items.is_empty() {
            widgets::skeleton(ui, 5, self.theme.row_h);
            return;
        }
        if self.fav_items.is_empty() {
            widgets::empty_state(
                ui,
                icons::folder,
                "这个收藏夹里没有可播放的视频",
                "收藏夹里的图文、番剧和付费内容无法提取音轨，因此不会出现在这里。",
                &[],
                accent,
            );
            return;
        }
        self.pane_body(ui, PaneSource::Favorites);
    }

    /// The horizontally scrolling row of favourite folders.
    fn folder_bar(&mut self, ui: &mut Ui, accent: t::Accent) {
        let mut pick: Option<i64> = None;
        let folders = self.fav_folders.clone();
        let selected = self.selected_folder;

        ui.add_space(11.0);
        ui.horizontal(|ui| {
            ui.add_space(22.0);
            ui.label(RichText::new("文件夹").size(11.5).color(t::FG_3));
            ui.add_space(4.0);
            egui::ScrollArea::horizontal()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for folder in &folders {
                            let on = Some(folder.id) == selected;
                            if folder_chip(ui, &folder.title, folder.media_count, on, accent)
                                .clicked()
                            {
                                pick = Some(folder.id);
                            }
                            ui.add_space(1.0);
                        }
                    });
                });
        });
        ui.add_space(9.0);
        super::hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);

        if let Some(id) = pick {
            if Some(id) != selected {
                self.selected_folder = Some(id);
                self.fav_items.clear();
                self.fav_page = 1;
                self.fav_loaded = false;
                // Any page still in flight belongs to the folder just left.
                self.loading_more = false;
                self.send(crate::net::Cmd::LoadFavItems {
                    media_id: id,
                    page: 1,
                });
            }
        }
    }

    pub(crate) fn ui_history(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;

        if self.user.is_none() {
            list_header(ui, "历史", "", |_ui| {});
            widgets::empty_state(
                ui,
                icons::clock,
                "登录后可查看观看历史",
                "历史记录只包含视频类内容，音频投稿不会出现在这里。",
                &[],
                accent,
            );
            ui.vertical_centered(|ui| {
                if primary_button(ui, "扫码登录", Some(icons::qr), accent).clicked() {
                    self.open_login();
                }
            });
            return;
        }

        let count = self.history.len();
        let mut refresh = false;
        list_header(ui, "历史", &format!("{count} 条记录"), |ui| {
            if ghost_button(ui, "刷新历史", Some(icons::refresh), accent, false, true).clicked()
            {
                refresh = true;
            }
        });
        if refresh {
            self.history_loaded = false;
            self.send(crate::net::Cmd::LoadHistory);
        }

        if !self.history_loaded && self.history.is_empty() {
            widgets::skeleton(ui, 5, self.theme.row_h);
            return;
        }
        if self.history.is_empty() {
            widgets::empty_state(
                ui,
                icons::clock,
                "还没有观看记录",
                "历史只记录视频类内容，音频投稿不会出现在这里。",
                &[],
                accent,
            );
            return;
        }
        self.pane_body(ui, PaneSource::History);
    }
}

/// One folder chip from the design's `.folderbar`.
fn folder_chip(
    ui: &mut Ui,
    title: &str,
    count: i64,
    active: bool,
    accent: t::Accent,
) -> egui::Response {
    let font = t::ui_font(12.5);
    let count_text = count.to_string();
    let width = widgets::measure(ui, title, font.clone()).x
        + widgets::measure(ui, &count_text, t::mono_font(10.5)).x
        + 13.0
        + 7.0
        + 6.0
        + 22.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered();

    painter.rect_filled(
        rect,
        t::R_SM,
        if active {
            t::INK_5
        } else if hovered {
            t::white(0.055)
        } else {
            Color32::TRANSPARENT
        },
    );
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, if active { t::LINE_2 } else { t::LINE }),
        egui::StrokeKind::Inside,
    );

    icons::folder(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 11.0, rect.center().y - 6.5),
            Vec2::splat(13.0),
        ),
        if active { accent.accent } else { t::FG_2 },
    );
    let mut x = rect.left() + 11.0 + 13.0 + 7.0;
    widgets::paint_label(
        &painter,
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        font.clone(),
        if active { Color32::WHITE } else { t::FG_2 },
    );
    x += widgets::measure(ui, title, &font).x + 6.0;
    widgets::paint_label(
        &painter,
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        count_text,
        t::mono_font(10.5),
        if active { accent.ink } else { t::FG_3 },
    );

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

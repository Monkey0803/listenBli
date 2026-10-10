//! The QR-code login modal.

use egui::{Align2, Color32, Id, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use super::icons;
use super::theme::{self as t};
use super::widgets;
use crate::app::App;

/// Which of the four designed QR states we are in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QrState {
    Waiting,
    Scanned,
    Done,
    Expired,
}

enum Veil {
    Spinner,
    Check,
    Refresh,
}

impl App {
    pub(crate) fn ui_login_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.qr.as_ref() else {
            return;
        };
        let state = if dialog.message.contains("过期") {
            QrState::Expired
        } else if dialog.message.contains("成功") {
            QrState::Done
        } else if dialog.message.contains("确认") || dialog.message.contains("已扫码") {
            QrState::Scanned
        } else {
            QrState::Waiting
        };
        let message = dialog.message.clone();
        let texture = dialog.texture.clone();

        let accent = self.theme.accent;
        let mut refresh = false;
        let mut close = false;

        let modal = egui::Modal::new(Id::new("login-modal"))
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_2)
                    .corner_radius(t::R_XL)
                    .inner_margin(Margin::same(26))
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .backdrop_color(Color32::from_black_alpha(158))
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("扫码登录").size(17.0).color(t::FG));
                    ui.add_space(7.0);
                    widgets::centred_paragraph(
                        ui,
                        "使用哔哩哔哩手机客户端扫码，凭据会加密保存在本机，重启后仍然有效。",
                        t::ui_font(12.5),
                        t::FG_3,
                        360.0,
                        1.7,
                    );
                    ui.add_space(22.0);

                    qr_card(ui, ctx, state, accent, texture.as_ref());

                    ui.add_space(14.0);
                    ui.label(RichText::new(message.as_str()).size(12.5).color(
                        if state == QrState::Expired {
                            t::WARN
                        } else {
                            t::FG_2
                        },
                    ));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("扫码后音质与「我的」内容将自动解锁")
                            .size(11.0)
                            .color(t::FG_3),
                    );

                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        let buttons = 110.0 * 2.0 + 10.0;
                        ui.add_space((400.0_f32.max(ui.available_width()) - buttons) * 0.5);
                        if super::ghost_button(
                            ui,
                            "刷新二维码",
                            Some(icons::refresh),
                            accent,
                            false,
                            true,
                        )
                        .clicked()
                        {
                            refresh = true;
                        }
                        ui.add_space(10.0);
                        if super::ghost_button(ui, "关闭", None, accent, false, true).clicked() {
                            close = true;
                        }
                    });

                    ui.add_space(18.0);
                    super::hline(ui, ui.min_rect().bottom(), t::LINE);
                    ui.add_space(14.0);

                    ui.vertical(|ui| {
                        tip(
                            ui,
                            "登录后解锁 FLAC 无损、「我的收藏夹」与观看历史。",
                            accent,
                            false,
                        );
                        ui.add_space(9.0);
                        tip(
                            ui,
                            "SESSDATA 等凭据只会写入本机配置文件，权限 0600。",
                            accent,
                            true,
                        );
                    });
                });
            });

        if modal.should_close() {
            close = true;
        }
        if refresh {
            self.send(crate::net::Cmd::QrStart);
            if let Some(dialog) = &mut self.qr {
                dialog.texture = None;
                dialog.image = None;
                dialog.message = "正在获取二维码…".into();
                dialog.finished = false;
                dialog.last_poll = std::time::Instant::now();
            }
        }
        if close {
            self.qr = None;
        }
    }
}

fn tip(ui: &mut egui::Ui, text: &str, accent: t::Accent, lock: bool) {
    let width = ui.available_width();
    let galley = ui.painter().layout(
        text.to_owned(),
        t::ui_font(12.0),
        t::FG_3,
        (width - 24.0).max(80.0),
    );
    let (row, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y), Sense::hover());
    let painter = ui.painter().clone();
    // The icon rides the first line, not the centre of a wrapped paragraph.
    let center_y = row.top() + widgets::first_row_ink_center(&galley);
    let icon_rect = Rect::from_min_size(Pos2::new(row.left(), center_y - 7.0), Vec2::splat(14.0));
    if lock {
        icons::lock(&painter, icon_rect, accent.accent);
    } else {
        icons::spark(&painter, icon_rect, accent.accent);
    }
    painter.galley(Pos2::new(row.left() + 24.0, row.top()), galley, t::FG_3);
}

/// The QR panel: a white card holding the code, with a scan sweep and the
/// state veil on top.
fn qr_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: QrState,
    accent: t::Accent,
    texture: Option<&egui::TextureHandle>,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(216.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, t::R_LG, Color32::WHITE);
    let inner = rect.shrink(12.0);

    match texture {
        Some(texture) => {
            painter.image(
                texture.id(),
                inner,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => placeholder_qr(&painter, inner),
    }

    // The state veil dims the code; the sweep only runs while waiting.
    match state {
        QrState::Waiting => {
            let time = ui.input(|i| i.time) as f32;
            let phase = (time * 0.42).rem_euclid(1.0);
            let band_h = rect.height() * 0.32;
            let band = Rect::from_min_size(
                Pos2::new(
                    rect.left() + 8.0,
                    rect.top() + 8.0 + phase * (rect.height() - band_h - 16.0),
                ),
                Vec2::new(rect.width() - 16.0, band_h),
            );
            t::gradient_rounded_rect(
                &painter,
                band,
                6.0,
                Color32::TRANSPARENT,
                t::fade(accent.accent, 0.32),
                Vec2::new(0.0, 1.0),
            );
            painter.rect_stroke(
                band,
                6.0,
                Stroke::new(1.0, t::fade(accent.accent, 0.5)),
                egui::StrokeKind::Inside,
            );
            ctx.request_repaint();
        }
        QrState::Scanned => veil(ui, &painter, rect, accent, Veil::Spinner),
        QrState::Done => veil(ui, &painter, rect, accent, Veil::Check),
        QrState::Expired => veil(ui, &painter, rect, accent, Veil::Refresh),
    }
}

/// A quiet module grid, so the card never looks empty while the API call is
/// still in flight.
fn placeholder_qr(painter: &egui::Painter, rect: Rect) {
    const CELLS: usize = 25;
    let step = rect.width() / CELLS as f32;
    for row in 0..CELLS {
        for col in 0..CELLS {
            let finder = (row < 7 && col < 7) || (row < 7 && col >= 18) || (row >= 18 && col < 7);
            let on = finder || ((row * 7 + col * 13) % 5) < 2;
            if !on {
                continue;
            }
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(
                        rect.left() + col as f32 * step,
                        rect.top() + row as f32 * step,
                    ),
                    Vec2::splat(step - 1.0),
                ),
                0.0,
                Color32::from_rgb(0x0b, 0x0b, 0x10),
            );
        }
    }
}

fn veil(ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect, accent: t::Accent, kind: Veil) {
    painter.rect_filled(
        rect,
        t::R_LG,
        Color32::from_rgba_unmultiplied(255, 255, 255, 235),
    );
    let center = rect.center();
    match kind {
        Veil::Spinner => {
            let time = ui.input(|i| i.time) as f32;
            let start = (time * 4.2) % std::f32::consts::TAU;
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let angle = start + i as f32 / 20.0 * std::f32::consts::TAU * 0.8;
                    center + Vec2::angled(angle) * 15.0
                })
                .collect();
            painter.line(points, Stroke::new(2.4, accent.accent));
            centered(painter, center.x, center.y + 34.0, "已扫码", t::INK_1, 12.5);
            centered(
                painter,
                center.x,
                center.y + 52.0,
                "请在手机上确认登录",
                MUTED_INK,
                11.0,
            );
            ui.ctx().request_repaint();
        }
        Veil::Check | Veil::Refresh => {
            let disc =
                Rect::from_center_size(Pos2::new(center.x, center.y - 8.0), Vec2::splat(54.0));
            let (from, to) = if matches!(kind, Veil::Refresh) {
                (
                    Color32::from_rgb(0x6b, 0x72, 0x80),
                    Color32::from_rgb(0x4b, 0x55, 0x63),
                )
            } else {
                (accent.accent, accent.deep)
            };
            t::gradient_rounded_rect(painter, disc, 27.0, from, to, Vec2::new(0.5, 1.0));
            let icon = if matches!(kind, Veil::Refresh) {
                icons::refresh
            } else {
                icons::check
            };
            icon(painter, disc.shrink(15.0), Color32::WHITE);

            let (title, sub) = if matches!(kind, Veil::Refresh) {
                ("二维码已过期", "点击刷新后重新扫码")
            } else {
                ("登录成功", "正在同步收藏夹与历史…")
            };
            centered(painter, center.x, center.y + 30.0, title, t::INK_1, 12.5);
            centered(painter, center.x, center.y + 48.0, sub, MUTED_INK, 11.0);
        }
    }
}

const MUTED_INK: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 128);

fn centered(painter: &egui::Painter, x: f32, y: f32, text: &str, color: Color32, size: f32) {
    widgets::paint_label(
        painter,
        Pos2::new(x, y),
        Align2::CENTER_CENTER,
        text,
        t::ui_font(size),
        color,
    );
}

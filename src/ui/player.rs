//! The bottom transport bar: what is playing, how to control it, and how far
//! the audio has been cached.

use egui::{Align, Align2, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::net::human_bytes;
use crate::util;

use super::icons;
use super::theme::{self as t};
use super::widgets;
use crate::app::App;

impl App {
    pub(crate) fn ui_player_bar(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let total = ui.available_width();
        let gap = 20.0;
        let usable = (total - gap * 2.0).max(220.0);
        // The side columns hold fixed-width controls (volume + queue chip on the
        // right, cover + title on the left), so they get a hard floor rather
        // than a pure fraction — otherwise the queue chip would overlap the
        // transport in a narrow window.
        let side = (usable * 0.25).clamp(276.0, 420.0);
        let middle = (usable - side * 2.0).max(200.0);

        // The design's bloom: a colour wash keyed to the accent, so the bar
        // reads as "on air" even when the cover is a placeholder.
        let rect = ui.max_rect();
        t::bloom(
            ui.painter(),
            Pos2::new(rect.left() + 120.0, rect.bottom() + 40.0),
            260.0,
            accent.accent,
            if self.current.is_some() { 0.24 } else { 0.06 },
        );

        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(side, 54.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(side);
                    self.now_playing(ui, accent)
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                Vec2::new(middle, 54.0),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.set_min_width(middle);
                    self.transport(ui)
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                Vec2::new(side, 54.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.set_min_width(side);
                    self.transport_right(ui, accent)
                },
            );
        });

        ui.add_space(5.0);
        self.player_sub(ui);
    }

    fn now_playing(&mut self, ui: &mut Ui, accent: t::Accent) {
        let current = self.current.clone();
        let quality = self.quality;
        let loading = self.loading;

        let (cover_rect, _) = ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
        widgets::paint_cover(
            ui.painter(),
            cover_rect,
            current.as_ref(),
            current
                .as_ref()
                .and_then(|track| self.covers.get(&track.key())),
        );

        ui.add_space(13.0);
        let text_rect = ui.available_rect_before_wrap();
        let painter = ui.painter().clone();

        let title_font = t::ui_font(14.5);
        let title_text = current
            .as_ref()
            .map(|track| track.title.clone())
            .unwrap_or_else(|| "未在播放".to_owned());
        let title_color = if current.is_some() { t::FG } else { t::FG_3 };
        let title = widgets::elided(ui, &title_text, title_font, title_color, text_rect.width());
        widgets::paint_galley_centred(
            &painter,
            Pos2::new(text_rect.left(), text_rect.center().y - 11.0),
            title,
            title_color,
        );

        let sub_font = t::ui_font(11.5);
        let sub_y = text_rect.center().y + 11.0;
        let Some(track) = current else {
            widgets::paint_label(
                &painter,
                Pos2::new(text_rect.left(), sub_y),
                Align2::LEFT_CENTER,
                "从左侧列表中选择一首开始",
                sub_font,
                t::FG_3,
            );
            return;
        };

        let author = if track.author.is_empty() {
            "未知作者"
        } else {
            track.author.as_str()
        };
        let author = widgets::elided(
            ui,
            author,
            sub_font.clone(),
            t::FG_2,
            (text_rect.width() - 70.0).max(40.0),
        );
        widgets::paint_galley_centred(
            &painter,
            Pos2::new(text_rect.left(), sub_y),
            author.clone(),
            t::FG_2,
        );
        let mut x = text_rect.left() + author.size().x + 7.0;
        painter.circle_filled(Pos2::new(x + 1.5, sub_y), 1.5, t::FG_4);
        x += 10.0;

        let label = if loading {
            "下载中…".to_owned()
        } else {
            quality
                .map(|quality| quality.label())
                .unwrap_or("未知音质")
                .to_owned()
        };
        widgets::paint_label(
            &painter,
            Pos2::new(x, sub_y),
            Align2::LEFT_CENTER,
            &label,
            sub_font.clone(),
            t::FG_3,
        );
        if quality == Some(crate::api::models::AudioQuality::Flac) {
            x += widgets::measure(ui, &label, sub_font).x + 6.0;
            icons::spark(
                &painter,
                Rect::from_min_size(Pos2::new(x, sub_y - 5.5), Vec2::splat(11.0)),
                t::GOLD,
            );
        }
        let _ = accent;
    }

    fn transport(&mut self, ui: &mut Ui) {
        let has_engine = self.engine.is_some();
        let loading = self.loading;
        let playing = self.is_playing();
        let has_queue = !self.queue.is_empty();
        let mut toggle = false;
        let mut prev = false;
        let mut next = false;

        ui.horizontal(|ui| {
            let buttons_w = 34.0 + 6.0 + 44.0 + 6.0 + 34.0;
            ui.add_space(((ui.available_width() - buttons_w) * 0.5).max(0.0));

            if transport_button(ui, icons::prev, 34.0, has_queue, "上一首").clicked() {
                prev = true;
            }
            ui.add_space(6.0);

            let (rect, response) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
            let painter = ui.painter().clone();
            let enabled = has_engine && !loading;
            let hovered = response.hovered() && enabled;
            let radius = if hovered { 23.0 } else { 22.0 };
            let ink = Color32::from_rgb(0x2a, 0x0d, 0x18);
            if enabled {
                t::gradient_rounded_rect(
                    &painter,
                    Rect::from_center_size(rect.center(), Vec2::splat(radius * 2.0)),
                    99.0,
                    Color32::from_rgb(0xff, 0xd3, 0xe0),
                    self.theme.accent.accent,
                    Vec2::new(0.4, 1.0),
                );
            } else {
                painter.circle_filled(rect.center(), radius, t::INK_4);
            }
            if loading {
                player_spinner(ui, rect.center());
            } else if playing {
                icons::pause(&painter, rect.shrink(12.0), ink);
            } else {
                icons::play(&painter, rect.shrink(12.0), ink);
            }
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() && enabled {
                toggle = true;
            }
            ui.add_space(6.0);

            if transport_button(ui, icons::next, 34.0, has_queue, "下一首").clicked() {
                next = true;
            }
        });

        if toggle {
            self.toggle_pause();
        }
        if prev {
            self.play_prev();
        }
        if next {
            self.play_next();
        }

        ui.add_space(2.0);

        // -- seek row -------------------------------------------------------
        let progress = self.progress();
        // With nothing downloading the whole track is available, which is what
        // the design shows for a fully cached song.
        let buffered = if self.download.is_some() {
            self.buffered()
        } else {
            Some(1.0)
        };
        let position = self.position();
        let duration = self.duration();
        let mut seek_to = None;

        ui.horizontal(|ui| {
            let time_font = t::mono_font(11.0);
            ui.label(
                RichText::new(util::format_duration(position.as_secs()))
                    .font(time_font.clone())
                    .color(t::FG_3),
            );
            ui.add_space(3.0);
            let width = (ui.available_width() - 86.0).max(60.0);
            let scrub = widgets::scrub(
                ui,
                progress,
                buffered,
                width,
                self.theme.accent,
                20.0,
                false,
            );
            if scrub.response.dragged() || scrub.response.clicked() {
                seek_to = Some(scrub.value);
            }
            ui.add_space(3.0);
            ui.label(
                RichText::new(util::format_duration(duration.as_secs()))
                    .font(time_font)
                    .color(t::FG_3),
            );
        });

        if let Some(value) = seek_to {
            self.try_seek_fraction(value);
        }
    }

    fn transport_right(&mut self, ui: &mut Ui, accent: t::Accent) {
        ui.add_space(10.0);

        // -- volume ---------------------------------------------------------
        let muted = self.volume <= 0.0;
        let mut new_volume = None;
        let mut persist = false;
        ui.allocate_ui_with_layout(
            Vec2::new(28.0 + 7.0 + 96.0, 28.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let icon = if muted {
                    icons::volume_muted
                } else {
                    icons::volume
                };
                if widgets::icon_button(ui, icon, 28.0, false, accent, "静音").clicked() {
                    new_volume = Some(if muted { 0.8 } else { 0.0 });
                }
                ui.add_space(7.0);
                let scrub = widgets::scrub(ui, self.volume, None, 96.0, accent, 20.0, true);
                if scrub.response.dragged() || scrub.response.clicked() {
                    new_volume = Some(scrub.value.clamp(0.0, 1.0));
                }
                if scrub.response.drag_stopped() {
                    persist = true;
                }
            },
        );
        if let Some(volume) = new_volume {
            self.volume = volume;
            if let Some(engine) = &mut self.engine {
                engine.set_volume(self.volume);
            }
        }
        if persist {
            self.persist_volume();
        }

        let (sep, _) = ui.allocate_exact_size(Vec2::new(13.0, 24.0), Sense::hover());
        ui.painter()
            .vline(sep.center().x, sep.y_range().shrink(2.0), t::hairline());

        // -- queue ----------------------------------------------------------
        self.queue_control(ui, accent);
    }

    fn queue_control(&mut self, ui: &mut Ui, accent: t::Accent) {
        let label = if self.queue.is_empty() {
            "播放列表".to_owned()
        } else {
            format!("播放列表 {}/{}", self.queue_pos + 1, self.queue.len())
        };
        let response = widgets::chip(
            ui,
            &label,
            Some(icons::queue),
            None,
            self.queue_open,
            accent,
        );
        if response.clicked() {
            self.queue_open = !self.queue_open;
        }

        let queue = self.queue.clone();
        let position = self.queue_pos;
        let mut jump = None;
        let mut close = false;

        egui::Popup::from_response(&response)
            .open(self.queue_open && !queue.is_empty())
            .align(egui::emath::RectAlign::TOP_END)
            .gap(10.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_3)
                    .corner_radius(t::R_LG)
                    .inner_margin(egui::Margin::ZERO)
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .width(380.0)
            .show(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(15.0);
                        // Painted as one row: the icon, the title and the count
                        // must all sit on the same optical centre.
                        let title_font = t::ui_font(13.0);
                        let count_font = t::mono_font(11.0);
                        let count = format!("{} 首", queue.len());
                        let title_width = widgets::measure(ui, "播放列表", &title_font).x;
                        let width =
                            19.0 + title_width + 2.0 + widgets::measure(ui, &count, &count_font).x;
                        let (header, _) =
                            ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
                        let painter = ui.painter().clone();
                        icons::queue(
                            &painter,
                            Rect::from_min_size(
                                Pos2::new(header.left(), header.center().y - 7.0),
                                Vec2::splat(14.0),
                            ),
                            accent.accent,
                        );
                        widgets::paint_label(
                            &painter,
                            Pos2::new(header.left() + 19.0, header.center().y),
                            Align2::LEFT_CENTER,
                            "播放列表",
                            title_font,
                            t::FG,
                        );
                        widgets::paint_label(
                            &painter,
                            Pos2::new(header.left() + 19.0 + title_width + 2.0, header.center().y),
                            Align2::LEFT_CENTER,
                            &count,
                            count_font,
                            t::FG_4,
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(8.0);
                            if widgets::icon_button(ui, icons::close, 28.0, false, accent, "收起")
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(11.0);
                    super::hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);

                    egui::ScrollArea::vertical()
                        .max_height(372.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    for (index, track) in queue.iter().enumerate() {
                                        let is_current = index == position;
                                        let (rect, row) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), 32.0),
                                            Sense::click(),
                                        );
                                        let painter = ui.painter().clone();
                                        if is_current {
                                            painter.rect_filled(rect, t::R_SM, accent.dim());
                                        } else if row.hovered() {
                                            painter.rect_filled(rect, t::R_SM, t::white(0.06));
                                        }
                                        widgets::paint_label(
                                            &painter,
                                            Pos2::new(rect.left() + 11.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            if is_current {
                                                "▶".to_owned()
                                            } else {
                                                format!("{:02}", index + 1)
                                            },
                                            t::mono_font(10.5),
                                            if is_current { accent.accent } else { t::FG_3 },
                                        );
                                        widgets::clipped_line(
                                            ui,
                                            Rect::from_min_max(
                                                Pos2::new(rect.left() + 32.0, rect.top()),
                                                Pos2::new(rect.right() - 52.0, rect.bottom()),
                                            ),
                                            &track.title,
                                            t::ui_font(12.5),
                                            if is_current { accent.ink } else { t::FG_2 },
                                            false,
                                        );
                                        widgets::paint_label(
                                            &painter,
                                            Pos2::new(rect.right() - 9.0, rect.center().y),
                                            Align2::RIGHT_CENTER,
                                            util::format_duration(track.duration),
                                            t::mono_font(10.5),
                                            t::FG_3,
                                        );
                                        if row.clicked() {
                                            jump = Some(index);
                                        }
                                        if row.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                    }
                                });
                            });
                            ui.add_space(6.0);
                        });
                });
            });

        if close {
            self.queue_open = false;
        }
        if let Some(index) = jump {
            let queue = self.queue.clone();
            self.play_track(queue[index].clone(), queue, index);
        }
    }

    /// The line under the transport: cache progress, or the keyboard hint.
    fn player_sub(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            match self.download.clone() {
                Some((_, got, total)) => {
                    let ratio = total
                        .filter(|total| *total > 0)
                        .map(|total| (got as f32 / total as f32).clamp(0.0, 1.0));
                    let width = (ui.available_width() * 0.62).min(520.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
                    let painter = ui.painter().clone();
                    let track = Rect::from_min_size(
                        Pos2::new(rect.left(), rect.center().y - 1.5),
                        Vec2::new((rect.width() - 160.0).max(60.0), 3.0),
                    );
                    painter.rect_filled(track, 1.5, t::white(0.09));
                    if let Some(ratio) = ratio {
                        t::gradient_rounded_rect(
                            &painter,
                            Rect::from_min_size(
                                track.min,
                                Vec2::new(track.width() * ratio, track.height()),
                            ),
                            1.5,
                            t::BLUE,
                            self.theme.accent.accent,
                            Vec2::new(1.0, 0.0),
                        );
                    }
                    let text = match ratio {
                        Some(ratio) => format!(
                            "缓存音频 {}% · {}{}",
                            (ratio * 100.0) as u32,
                            human_bytes(got),
                            total
                                .map(|t| format!(" / {}", human_bytes(t)))
                                .unwrap_or_default()
                        ),
                        None => format!("缓存音频 · {}", human_bytes(got)),
                    };
                    widgets::paint_label(
                        &painter,
                        Pos2::new(track.right() + 10.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        text,
                        t::mono_font(10.5),
                        t::FG_3,
                    );
                }
                None => {
                    let hint = if self.current.is_some() {
                        "空格 播放 / 暂停 · ← → 快退快进 5 秒 · 点击歌词行跳转"
                    } else {
                        "音频会先缓存到本地再播放，通常 1–7 MB"
                    };
                    ui.label(RichText::new(hint).size(11.0).color(t::FG_3));
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(err) = &self.engine_error {
                    ui.label(RichText::new(err).size(11.0).color(t::ERR));
                }
            });
        });
    }

    fn persist_volume(&self) {
        if let Ok(mut guard) = self.config.lock() {
            guard.volume = self.volume;
            if let Err(err) = guard.save() {
                eprintln!("saving volume failed: {err}");
            }
        }
    }
}

fn transport_button(
    ui: &mut Ui,
    icon: icons::Icon,
    size: f32,
    enabled: bool,
    tip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter().clone();
    let hovered = response.hovered() && enabled;
    if hovered {
        painter.circle_filled(rect.center(), size * 0.5, t::white(0.08));
    }
    icon(
        &painter,
        rect.shrink(size * 0.24),
        if !enabled {
            t::FG_4
        } else if hovered {
            Color32::WHITE
        } else {
            t::FG_2
        },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if enabled {
        response.on_hover_text(tip)
    } else {
        response
    }
}

/// Draws the rotating arc used on the play button while a track downloads.
fn player_spinner(ui: &Ui, center: Pos2) {
    let painter = ui.painter().clone();
    let time = ui.input(|i| i.time) as f32;
    let start = (time * 4.2) % std::f32::consts::TAU;
    let points: Vec<Pos2> = (0..=18)
        .map(|i| {
            let angle = start + i as f32 / 18.0 * std::f32::consts::TAU * 0.8;
            center + Vec2::angled(angle) * 8.0
        })
        .collect();
    painter.line(
        points,
        Stroke::new(2.0, Color32::from_rgb(0x2a, 0x0d, 0x18)),
    );
    ui.ctx().request_repaint();
}

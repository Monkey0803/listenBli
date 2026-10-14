//! The bottom transport bar: what is playing, how to control it, and how far
//! the audio has been cached.

use egui::{Align, Align2, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::net::human_bytes;
use crate::util;

use super::icons;
use super::songs;
use super::theme::{self as t};
use super::widgets;
use crate::app::App;

impl App {
    pub(crate) fn ui_player_bar(&mut self, ui: &mut Ui) {
        // A panel's frame reports only the rect its content actually *used*, and
        // the neighbouring panels are inset by exactly that — so a deck shorter
        // than its strip silently shrinks the strip, and the rail and the page
        // then draw over the deck's top. Allocating the whole strip up front is
        // what makes `exact_size(DECK_H)` true for everyone else too.
        let strip = ui.available_size();
        ui.allocate_ui_with_layout(strip, Layout::top_down(Align::Min), |ui| {
            self.player_bar_body(ui);
        });
    }

    fn player_bar_body(&mut self, ui: &mut Ui) {
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

        // The deck is 96px tall and its content is ~79px, so centre it: the layout
        // would otherwise pin everything to the top edge, which read as a clipped
        // bar once the plate grew from the old 78px.
        const CONTENT_H: f32 = 56.0;
        let slack = (ui.available_height() - CONTENT_H).max(0.0);
        ui.add_space(slack / 2.0);

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
        let title_w = title.size().x;
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

        // Quality and 已回退 ride on the title's line as tags. The quality used
        // to be plain text in the sub-line, where it read as a property of the
        // uploader rather than of the song.
        let title_cy = text_rect.center().y - 11.0;
        let mut tag_x = text_rect.left() + title_w + 9.0;
        if !loading {
            if let Some(quality) = quality {
                let size = widgets::tag_size(ui, quality.label(), None, false);
                let rect = Rect::from_center_size(Pos2::new(tag_x + size.x / 2.0, title_cy), size);
                widgets::tag_at(ui, rect, quality.label(), None, None, None, accent);
                tag_x += size.x + 6.0;
            }
        }
        // 已回退 is a warning, so it is the one tag in red: the copy that was meant
        // to play is gone and another platform's is standing in.
        if !loading && self.is_fallback(&track) {
            let size = widgets::tag_size(ui, "已回退", None, false);
            let rect = Rect::from_center_size(Pos2::new(tag_x + size.x / 2.0, title_cy), size);
            widgets::tag_at(ui, rect, "已回退", None, None, Some(t::RED), accent);
        }

        // Sub-line: artist · album, then the way into 来源.
        let source_size = widgets::tag_size(ui, "来源", None, false);
        let subtitle = songs::subtitle(&track);
        let subtitle = widgets::elided(
            ui,
            &subtitle,
            sub_font.clone(),
            t::FG_3,
            (text_rect.width() - source_size.x - 16.0).max(40.0),
        );
        let subtitle_w = subtitle.size().x;
        widgets::paint_galley_centred(
            &painter,
            Pos2::new(text_rect.left(), sub_y),
            subtitle,
            t::FG_3,
        );

        let source_rect = Rect::from_min_size(
            Pos2::new(text_rect.left() + subtitle_w + 10.0, sub_y - 10.0),
            source_size,
        );
        let response = ui.interact(
            source_rect,
            ui.make_persistent_id(("deck-source", track.key())),
            Sense::click(),
        );
        widgets::tag_at(
            ui,
            source_rect,
            "来源",
            Some(icons::chevron_right),
            None,
            if response.hovered() {
                Some(accent.accent)
            } else {
                None
            },
            accent,
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            self.source_open = true;
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
        self.streaming_box(ui, accent);
        ui.add_space(12.0);

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

        // Size the list to the room above the chip so the popup can never be
        // clipped by the window edge, then hold it inside the bounds above.
        let space_above = response.rect.top() - ui.ctx().viewport_rect().top() - 84.0;
        let list_h = space_above.clamp(QUEUE_LIST_MIN_H, QUEUE_LIST_MAX_H);

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
                    if queue_header(ui, accent, queue.len()) {
                        close = true;
                    }
                    ui.add_space(11.0);
                    super::hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);

                    // `list_h` is both the floor and the ceiling: with auto
                    // shrink off, the area takes exactly the height it is given,
                    // so a short queue no longer collapses to its content.
                    egui::ScrollArea::vertical()
                        .max_height(list_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    for (index, track) in queue.iter().enumerate() {
                                        let is_current = index == position;
                                        let (rect, row) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), QUEUE_ROW_H),
                                            Sense::click(),
                                        );
                                        let painter = ui.painter().clone();
                                        if is_current {
                                            painter.rect_filled(rect, t::R_SM, accent.dim());
                                        } else if row.hovered() {
                                            painter.rect_filled(rect, t::R_SM, t::white(0.06));
                                        }
                                        if is_current {
                                            // A state marker, not a control: the
                                            // text glyph this used to paint read as
                                            // "press to play" on the row that is
                                            // already playing.
                                            icons::pause(
                                                &painter,
                                                Rect::from_center_size(
                                                    Pos2::new(rect.left() + 15.0, rect.center().y),
                                                    Vec2::splat(13.0),
                                                ),
                                                accent.accent,
                                            );
                                        } else {
                                            widgets::paint_label(
                                                &painter,
                                                Pos2::new(rect.left() + 11.0, rect.center().y),
                                                Align2::LEFT_CENTER,
                                                format!("{:02}", index + 1),
                                                t::mono_font(10.5),
                                                t::FG_3,
                                            );
                                        }
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
    /// The design's small streaming box: how much of the track is on its way.
    ///
    /// This is where the cache readout lives now. The old bar drew it as a
    /// full-width row underneath itself, which made the deck's content taller than
    /// the deck — and because a panel reports only the height its content used,
    /// the overflow pushed the panel's own rect down and the page beside it drew
    /// over the deck's top edge. Folding it into the right group keeps the
    /// information and the 96px strip.
    fn streaming_box(&mut self, ui: &mut Ui, _accent: t::Accent) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0, 44.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_filled(rect, t::R_SM, t::INK_1);
        painter.rect_stroke(
            rect,
            t::R_SM,
            Stroke::new(1.0, t::LINE),
            egui::StrokeKind::Inside,
        );
        widgets::engraved_at(
            ui,
            Pos2::new(rect.left() + 10.0, rect.top() + 14.0),
            Align2::LEFT_CENTER,
            "STREAMING",
            9.0,
            t::FG_4,
        );

        let (text, ratio) = match self.download.clone() {
            Some((_, got, total)) => {
                let ratio = total
                    .filter(|total| *total > 0)
                    .map(|total| (got as f32 / total as f32).clamp(0.0, 1.0));
                let text = match (ratio, total) {
                    (Some(ratio), Some(total)) => format!(
                        "{}% · {} / {}",
                        (ratio * 100.0) as u32,
                        human_bytes(got),
                        human_bytes(total)
                    ),
                    _ => human_bytes(got),
                };
                (text, ratio.unwrap_or(0.0))
            }
            None if self.current.is_some() => ("已就绪".to_owned(), 1.0),
            None => ("—".to_owned(), 0.0),
        };
        widgets::label_at(
            ui,
            Pos2::new(rect.right() - 10.0, rect.top() + 14.0),
            Align2::RIGHT_CENTER,
            &text,
            t::mono_font(10.0),
            t::FG_2,
        );

        let track = Rect::from_min_size(
            Pos2::new(rect.left() + 10.0, rect.bottom() - 13.0),
            Vec2::new(rect.width() - 20.0, 3.0),
        );
        t::level_bar(&painter, track, ratio, self.theme.accent.accent);
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

/// The queue popup's header row, returning `true` when 收起 was clicked.
///
/// Everything here is painted as one row so the icon, the title, the count and
/// the unit sit on one optical centre. The count is deliberately two labels:
/// epaint positions a mixed Latin/CJK run by centring each face's ascent box,
/// which floats 首 a couple of pixels above the mono digits and drags those
/// digits below the centre line. Painted separately, each piece is centred on
/// its own ink, like every other hand-painted label in the chrome.
/// One row in the queue popup.
const QUEUE_ROW_H: f32 = 32.0;

/// The list is never shorter than this, so a three-song queue still reads as a
/// panel rather than a stub — which is what it looked like when the scroll area
/// shrank to its content.
const QUEUE_LIST_MIN_H: f32 = 198.0;

/// Nor taller than this, so the popup cannot run off the top of a short window.
///
/// It is a cap, not a promise: measured, `egui`'s popup area hands the list about
/// 341 px (nine rows) whether the window is 600 px tall or 1400, so a long queue
/// ends up there. Only a window shorter than that runs into this constant.
const QUEUE_LIST_MAX_H: f32 = 460.0;

fn queue_header(ui: &mut Ui, accent: t::Accent, count: usize) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.add_space(15.0);

        let title_font = t::ui_font(13.0);
        let count_font = t::mono_font(11.0);
        let count = count.to_string();
        let unit = "首";
        let title_width = widgets::measure(ui, "播放列表", &title_font).x;
        let count_width = widgets::measure(ui, &count, &count_font).x;
        let space = widgets::measure(ui, " ", &count_font).x;
        let width = 19.0
            + title_width
            + 2.0
            + count_width
            + space
            + widgets::measure(ui, unit, &count_font).x;
        let (header, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
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
        let mut x = header.left() + 19.0 + title_width + 2.0;
        for text in [&count, unit] {
            widgets::paint_label(
                &painter,
                Pos2::new(x, header.center().y),
                Align2::LEFT_CENTER,
                text,
                count_font.clone(),
                t::FG_4,
            );
            x += widgets::measure(ui, text, &count_font).x + space;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if widgets::icon_button(ui, icons::close, 28.0, false, accent, "收起").clicked() {
                close = true;
            }
        });
    });
    close
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

#[cfg(test)]
mod tests {
    //! The queue header is hand-painted, so this drives the real row and then
    //! inspects what egui actually painted.

    use super::*;
    use crate::config::Config;
    use egui::{RawInput, Rect, Vec2};

    /// Every string egui *actually shows*, with the y of its glyph ink's centre.
    ///
    /// Clip-aware on purpose: a shape scrolled out of a `ScrollArea` is still in
    /// the shape list, it just is not visible. Counting it would make any test
    /// about "is this row on screen" pass on content nobody can see.
    fn painted_ink(output: &egui::FullOutput) -> Vec<(String, f32)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, f32)>) {
            match shape {
                egui::Shape::Text(text) => out.push((
                    text.galley.text().to_owned(),
                    text.pos.y + text.galley.mesh_bounds.center().y,
                )),
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
            let mut found = Vec::new();
            walk(&clipped.shape, &mut found);
            let clip = clipped.clip_rect;
            if !clip.intersects(clipped.shape.visual_bounding_rect()) {
                continue;
            }
            out.extend(
                found
                    .into_iter()
                    .filter(|(_, y)| *y >= clip.top() && *y <= clip.bottom()),
            );
        }
        out
    }

    /// App a queue and render the player bar with the popup open, then report
    /// what is visible: the header's y and every queue row's y.
    fn queue_popup(count: usize) -> (Vec<(String, f32)>, f32, f32, Vec<egui::epaint::RectShape>) {
        crate::platform::serialise_home_tests();
        std::env::set_var("HOME", "/tmp/listenbli-queue-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-queue-tests");

        let mut app = App::new_for_tests(Config::default());
        app.queue = (0..count)
            .map(|i| crate::api::models::Track {
                bvid: format!("BV{i}"),
                source: crate::api::models::Source::Bilibili,
                aid: 0,
                cid: 0,
                title: format!("队列曲目 {i}"),
                author: "上传者".into(),
                duration: 200,
                cover: None,
                album: None,
                artist: None,
            })
            .collect();
        app.queue_pos = 0;
        app.queue_open = true;

        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);

        let mut last = None;
        for _ in 0..3 {
            let mut out = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0))),
                    ..Default::default()
                },
                |ui| {
                    egui::Panel::bottom("player_bar")
                        .frame(crate::ui::frames::player_bar(&app.theme))
                        .show(ui, |ui| app.ui_player_bar(ui));
                },
            );
            out.textures_delta.clear();
            last = Some(out);
        }
        let out = last.unwrap();

        let ink = painted_ink(&out);
        let chip_y = ink
            .iter()
            .find(|(text, _)| text.starts_with("播放列表 1/"))
            .map(|(_, y)| *y)
            .expect("the chip is painted");
        let header_y = ink
            .iter()
            .find(|(text, _)| text == "播放列表")
            .map(|(_, y)| *y)
            .expect("the popup header is painted");
        (ink, chip_y, header_y, painted_rects(&out))
    }

    /// Filled rects, clip-aware like `painted_ink` (the one in `ui::tests` is
    /// private to that module).
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

    /// The queue rows among the painted strings.
    fn queue_rows(ink: &[(String, f32)]) -> Vec<(String, f32)> {
        ink.iter()
            .filter(|(text, _)| text.starts_with("队列曲目"))
            .cloned()
            .collect()
    }

    /// Filled rects small enough to be the meter's bars.
    fn meter_bars(rects: &[egui::epaint::RectShape]) -> usize {
        rects
            .iter()
            .filter(|shape| {
                shape.rect.width() <= 4.0
                    && shape.rect.height() <= 12.0
                    && shape.rect.height() >= 3.0
            })
            .count()
    }

    /// The popup used to shrink to its content, so a three-song queue came out a
    /// ~120px stub floating in the middle of the window.
    ///
    /// Measured from the chip up to the header: that spans the header, the list
    /// and its padding, so requiring the list's minimum height of it is the
    /// weaker claim — and it is still one the old code failed (it managed 183px
    /// against the 198 asked for here).
    #[test]
    fn a_short_queue_still_gets_a_full_height_panel() {
        let (ink, chip_y, header_y, _) = queue_popup(3);
        let rows = queue_rows(&ink);
        assert_eq!(rows.len(), 3, "all three rows should be visible: {rows:?}");
        let panel_h = chip_y - header_y;
        assert!(
            panel_h >= QUEUE_LIST_MIN_H,
            "a three-song queue left only {panel_h:.0}px of panel, want >= {QUEUE_LIST_MIN_H}"
        );
    }

    /// The queue's playing row must not carry a play glyph.
    ///
    /// It used to paint the text character `▶`, which reads as "press to play" on
    /// the row that is already playing — and, being a font glyph, sat oddly beside
    /// a hand-painted icon set. It shows the level meter instead.
    #[test]
    fn the_queue_playing_row_offers_to_play_nowhere() {
        let (ink, _, _, rects) = queue_popup(3);
        let glyphs: Vec<&String> = ink
            .iter()
            .map(|(text, _)| text)
            .filter(|text| text.contains('▶') || text.contains('⏸'))
            .collect();
        assert!(
            glyphs.is_empty(),
            "no transport glyph belongs on a queue row: {glyphs:?}"
        );
        assert!(
            meter_bars(&rects) >= 2,
            "the pause glyph's two bars should be painted for the playing row, found {}",
            meter_bars(&rects)
        );
    }

    /// A long queue fills the panel and never spills behind the player bar.
    ///
    /// Nine rows is what fits: `egui`'s popup area offers about 341 px of list no
    /// matter how tall the window is, so a taller request cannot be honoured from
    /// in here. What must hold is that the rows shown are the ones above the bar
    /// and that the rest scroll — never that rows are painted out of sight.
    #[test]
    fn a_long_queue_fills_the_panel_without_spilling_behind_the_bar() {
        let (ink, chip_y, _, _) = queue_popup(20);
        let rows = queue_rows(&ink);
        assert!(
            rows.len() >= 9,
            "a 20-song queue should fill the panel, got {} rows: {rows:?}",
            rows.len()
        );
        assert!(
            rows.len() < 20,
            "the rest should scroll rather than all be laid out: {}",
            rows.len()
        );
        let lowest = rows.iter().map(|(_, y)| *y).fold(f32::MIN, f32::max);
        assert!(
            lowest < chip_y,
            "every visible row should sit above the chip, lowest was {lowest} vs {chip_y}"
        );
    }

    /// The header's icon, title, count and unit must share one centre line.
    ///
    /// Painting "20 首" as a single label failed this: epaint centres a mixed
    /// Latin/CJK run per font face, which floats 首 above the mono digits and
    /// sinks the digits below the line.
    #[test]
    fn the_queue_header_shares_one_centre_line() {
        crate::platform::serialise_home_tests();
        std::env::set_var("HOME", "/tmp/listenbli-queue-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-queue-tests");
        let app = App::new_for_tests(Config::default());
        let ctx = egui::Context::default();
        let cjk = crate::config::with(&app.config, |c| {
            crate::platform::resolve_cjk_font(&c.cjk_font, c.cjk_font_path.as_deref())
        });
        crate::ui::theme::install_fonts(&ctx, cjk.as_deref());
        crate::ui::theme::install_style(&ctx, &app.theme);
        let accent = app.theme.accent;

        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(420.0, 60.0))),
                ..Default::default()
            },
            |ui| {
                queue_header(ui, accent, 20);
            },
        );
        output.textures_delta.clear();

        let ink = painted_ink(&output);
        let centre = |label: &str| {
            ink.iter()
                .find(|(text, _)| text == label)
                .map(|(_, y)| *y)
                .unwrap_or_else(|| panic!("{label:?} should be painted: {ink:?}"))
        };
        let title = centre("播放列表");
        for label in ["20", "首"] {
            assert!(
                (centre(label) - title).abs() < 0.5,
                "{label:?} is centred at {}, but \"播放列表\" is at {title}",
                centre(label)
            );
        }
    }
}

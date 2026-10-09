//! The scrolling lyrics panel.
//!
//! Follow mode auto-centres the current line. Any manual scroll suspends
//! following for a few seconds so the reader can look ahead, and clicking a line
//! seeks to it — both standard music-player affordances.

use std::time::{Duration, Instant};

use egui::{Align, Align2, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use super::icons;
use super::theme::{self as t, Accent};
use super::widgets;
use crate::app::App;

/// How long a manual scroll suspends auto-follow.
const FOLLOW_SUSPEND: Duration = Duration::from_secs(3);

impl App {
    pub(crate) fn ui_lyrics_panel(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let lyric_size = self.theme.lyric_size;

        // The colour wash behind the header, keyed to the accent.
        let rect = ui.max_rect();
        t::bloom(
            ui.painter(),
            Pos2::new(rect.center().x, rect.top() - 60.0),
            230.0,
            accent.accent,
            if self.current.is_some() { 0.28 } else { 0.10 },
        );
        ui.painter()
            .vline(rect.left() + 0.5, rect.y_range(), t::hairline());

        self.lyrics_header(ui, accent);
        self.lyrics_meta(ui, accent);

        if self.lyrics.is_empty() {
            let (icon, title, desc): (icons::Icon, &str, &str) = if self.current.is_none() {
                (
                    icons::lyrics,
                    "还没有在播放",
                    "从左侧选一首开始，歌词会自动跟随高亮、居中滚动。点击任意一行可以跳到那一句。",
                )
            } else if self.loading {
                (
                    icons::lyrics,
                    "正在获取歌词",
                    "先匹配 B 站 CC 字幕，再回退到网易云音乐；两者都没有时会显示为无歌词。",
                )
            } else {
                (
                    icons::translate,
                    "暂无歌词",
                    "这个视频没有 CC 字幕，也没能在网易云匹配到可用的歌词。合集与纯音乐类投稿通常都没有。",
                )
            };
            widgets::empty_state(ui, icon, title, desc, &[], accent);
            return;
        }

        self.lyrics_body(ui, accent, lyric_size);
    }

    fn lyrics_header(&mut self, ui: &mut Ui, accent: Accent) {
        ui.add_space(15.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            // Painted, so the icon shares the label's ink centre rather than
            // the font's ascent/descent box.
            let title_font = t::ui_font(13.5);
            let width = 21.0 + widgets::measure(ui, "歌词", &title_font).x;
            let (title, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
            let painter = ui.painter().clone();
            icons::lyrics(
                &painter,
                Rect::from_min_size(
                    Pos2::new(title.left(), title.center().y - 7.5),
                    Vec2::splat(15.0),
                ),
                t::FG_3,
            );
            widgets::paint_label(
                &painter,
                Pos2::new(title.left() + 21.0, title.center().y),
                Align2::LEFT_CENTER,
                "歌词",
                title_font,
                t::FG,
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(18.0);
                let mut show_translation = self.show_translation;
                if mini_toggle(ui, "翻译", icons::translate, show_translation, accent).clicked() {
                    show_translation = !show_translation;
                    self.show_translation = show_translation;
                    self.update_config(|config| config.prefer_translation = show_translation);
                }
                ui.add_space(4.0);
                let follow = self.follow_lyrics;
                if mini_toggle(ui, "跟随", icons::target, follow, accent).clicked() {
                    self.follow_lyrics = !follow;
                    if !follow {
                        self.manual_scroll_at = None;
                    }
                }
            });
        });
        ui.add_space(11.0);
        super::hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);
    }

    fn lyrics_meta(&mut self, ui: &mut Ui, accent: Accent) {
        let source = self.lyrics.source;
        let lines = self.lyrics.lines.len();
        ui.add_space(9.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(RichText::new("来源").size(11.0).color(t::FG_3));
            source_badge(ui, source);
            if lines > 0 {
                ui.label(
                    RichText::new(format!("· {lines} 行"))
                        .size(11.0)
                        .color(t::FG_3),
                );
            }
            let _ = accent;
        });
        ui.add_space(5.0);
    }

    fn lyrics_body(&mut self, ui: &mut Ui, accent: Accent, lyric_size: f32) {
        let position = self.position();
        let current_index = self.lyrics.current_index(position);
        let follow = self.follow_lyrics;
        let show_translation = self.show_translation;
        let lines = std::mem::take(&mut self.lyrics.lines);

        let mut seek_to: Option<Duration> = None;
        let mut manual_scroll = false;
        let area = ui.available_rect_before_wrap();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(36.0);
                let width = ui.available_width();
                for (index, line) in lines.iter().enumerate() {
                    let is_current = Some(index) == current_index;
                    let is_near = current_index.is_some_and(|c| index.abs_diff(c) <= 2);

                    let main_font = t::lyric_font(if is_current {
                        lyric_size + 5.5
                    } else {
                        lyric_size
                    });
                    // Sung and not-yet-sung lines are deliberately quiet; the
                    // two lines around the active one get a lift.
                    let color = if is_current {
                        Color32::WHITE
                    } else if is_near {
                        t::FG_2
                    } else {
                        t::FG_3
                    };
                    let painter = ui.painter().clone();
                    let main = painter.layout(
                        line.text.clone(),
                        main_font,
                        color,
                        (width - 44.0).max(60.0),
                    );
                    let translation =
                        line.translation
                            .as_ref()
                            .filter(|_| show_translation)
                            .map(|text| {
                                painter.layout(
                                    text.clone(),
                                    t::ui_font(lyric_size - 2.5),
                                    if is_current { t::BLUE_INK } else { t::FG_3 },
                                    (width - 44.0).max(60.0),
                                )
                            });

                    let height = main.size().y
                        + translation
                            .as_ref()
                            .map(|galley| galley.size().y + 4.0)
                            .unwrap_or(0.0)
                        + 14.0;
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

                    if is_current {
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(rect.left(), rect.top() + 2.0),
                                Vec2::new(rect.width(), rect.height() - 4.0),
                            ),
                            t::R_SM,
                            t::white(0.035),
                        );
                    } else if response.hovered() {
                        painter.rect_filled(rect, t::R_SM, t::white(0.045));
                    }

                    if is_current {
                        let bar = Rect::from_center_size(
                            Pos2::new(rect.left() + 3.5, rect.center().y),
                            Vec2::new(3.0, rect.height() * 0.62),
                        );
                        t::gradient_rounded_rect(
                            &painter,
                            bar,
                            1.5,
                            accent.soft,
                            accent.accent,
                            Vec2::new(0.0, 1.0),
                        );
                    }

                    let mut y = rect.top() + 7.0;
                    let x = rect.left() + if is_current { 19.0 } else { 16.0 };
                    let main_height = main.size().y;
                    painter.galley(Pos2::new(x, y), main, color);
                    y += main_height;
                    if let Some(translation) = translation {
                        y += 4.0;
                        painter.galley(Pos2::new(x, y), translation, t::FG_3);
                    }

                    if response.clicked() {
                        seek_to = Some(line.time);
                    }
                    if is_current && follow {
                        response.scroll_to_me(Some(Align::Center));
                    }
                }
                ui.add_space(120.0);

                if ui.input(|input| input.smooth_scroll_delta.y.abs() > 0.5) {
                    manual_scroll = true;
                }
            });

        self.lyrics.lines = lines;

        if let Some(time) = seek_to {
            self.try_seek(time);
        }
        if manual_scroll {
            self.manual_scroll_at = Some(Instant::now());
        }
        if let Some(at) = self.manual_scroll_at {
            if at.elapsed() >= FOLLOW_SUSPEND {
                self.manual_scroll_at = None;
                self.follow_lyrics = true;
            }
        }

        // The "follow paused" pill floats over the bottom of the body.
        if !self.follow_lyrics {
            let text = "跟随已暂停 · 点击恢复";
            let font = t::ui_font(12.0);
            let width = widgets::measure(ui, text, &font).x + 44.0;
            let pill = Rect::from_center_size(
                Pos2::new(area.center().x, area.bottom() - 30.0),
                Vec2::new(width, 32.0),
            );
            let response = ui.interact(pill, ui.id().with("lyrics-resume"), Sense::click());
            let painter = ui.painter().clone();
            painter.rect_filled(
                pill,
                16.0,
                if response.hovered() {
                    t::INK_5
                } else {
                    t::INK_4
                },
            );
            painter.rect_stroke(
                pill,
                16.0,
                Stroke::new(1.0, t::LINE_2),
                egui::StrokeKind::Inside,
            );
            painter.circle_filled(Pos2::new(pill.left() + 17.0, pill.center().y), 2.5, t::WARN);
            widgets::paint_label(
                &painter,
                Pos2::new(pill.left() + 27.0, pill.center().y),
                Align2::LEFT_CENTER,
                text,
                font,
                t::FG,
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                self.follow_lyrics = true;
                self.manual_scroll_at = None;
            }
        }
    }
}

/// The lyrics panel's pill-shaped toggles.
fn mini_toggle(
    ui: &mut Ui,
    label: &str,
    icon: icons::Icon,
    on: bool,
    accent: Accent,
) -> egui::Response {
    let font = t::ui_font(11.5);
    let width = widgets::measure(ui, label, &font).x + 13.0 + 5.0 + 18.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 25.0), Sense::click());
    let painter = ui.painter().clone();

    let (fill, stroke, fg) = if on {
        (
            accent.dim(),
            Stroke::new(1.0, t::fade(accent.accent, 0.3)),
            accent.ink,
        )
    } else {
        (
            if response.hovered() {
                t::white(0.05)
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, t::LINE),
            if response.hovered() { t::FG_2 } else { t::FG_3 },
        )
    };
    widgets::paint_pill(ui, rect, fill, Some(stroke));
    icon(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 9.0, rect.center().y - 6.0),
            Vec2::splat(12.0),
        ),
        fg,
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + 9.0 + 5.0 + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        fg,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// The lyric-source badge (B station subtitles / NetEase / none).
fn source_badge(ui: &mut Ui, source: crate::lyrics::LyricsSource) {
    let (label, tone) = match source {
        crate::lyrics::LyricsSource::BilibiliSubtitle => ("B 站 CC 字幕", true),
        crate::lyrics::LyricsSource::Netease => ("网易云音乐", true),
        crate::lyrics::LyricsSource::None => ("无", false),
    };
    let font = t::ui_font(10.5);
    let width = widgets::measure(ui, label, &font).x + 14.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
    let painter = ui.painter().clone();
    if tone {
        painter.rect_filled(rect, 5.0, t::fade(t::BLUE, 0.13));
        painter.rect_stroke(
            rect,
            5.0,
            Stroke::new(1.0, t::fade(t::BLUE, 0.22)),
            egui::StrokeKind::Inside,
        );
        widgets::paint_label(
            &painter,
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font,
            t::BLUE_INK,
        );
    } else {
        painter.rect_filled(rect, 5.0, t::white(0.05));
        painter.rect_stroke(
            rect,
            5.0,
            Stroke::new(1.0, t::LINE),
            egui::StrokeKind::Inside,
        );
        widgets::paint_label(
            &painter,
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font,
            t::FG_3,
        );
    }
}

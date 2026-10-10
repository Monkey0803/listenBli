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

            // NetEase only ships a translation for some songs. When this lyric
            // has none the switch cannot change anything on screen, so the
            // click says so instead of looking broken.
            let has_lyrics = !self.lyrics.is_empty();
            let has_translation = self
                .lyrics
                .lines
                .iter()
                .any(|line| line.translation.is_some());

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(18.0);
                let mut show_translation = self.show_translation;
                let translate = mini_toggle(ui, "翻译", icons::translate, show_translation, accent);
                let translate = if has_translation {
                    translate.on_hover_text("原文与译文")
                } else if has_lyrics {
                    translate.on_hover_text("这首歌的歌词没有翻译")
                } else {
                    translate.on_hover_text("还没有歌词")
                };
                if translate.clicked() {
                    show_translation = !show_translation;
                    self.show_translation = show_translation;
                    self.update_config(|config| config.prefer_translation = show_translation);
                    if !has_translation {
                        let message = if has_lyrics {
                            "这首歌的歌词没有译文，切换不会改变显示"
                        } else {
                            "还没有歌词可显示翻译"
                        };
                        self.set_status(message, false);
                    }
                }
                ui.add_space(4.0);
                let follow = self.follow_lyrics;
                if mini_toggle(ui, "跟随", icons::target, follow, accent)
                    .on_hover_text("自动高亮并居中当前歌词行")
                    .clicked()
                {
                    self.follow_lyrics = !follow;
                    // Either way the pause ends: switching follow on resumes at
                    // once, switching it off must not leave a live timer behind.
                    self.manual_scroll_at = None;
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
        // The user's switch, minus any in-progress manual-scroll pause.
        let follow = self.follow_lyrics && self.manual_scroll_at.is_none();
        let show_translation = self.show_translation;
        let lines = std::mem::take(&mut self.lyrics.lines);

        let mut seek_to: Option<Duration> = None;
        let mut manual_scroll = false;
        let area = ui.available_rect_before_wrap();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Resolved before the lines are laid out, so the frame that
                // carries the wheel does not also re-centre on the current line.
                //
                // `smooth_scroll_delta` is global input, so it has to be gated
                // on the pointer being over the lyric body: scrolling the
                // history list used to pause — and then re-enable — follow.
                //
                // A drag counts too, which is what makes the scroll bar (and
                // egui's drag-to-scroll) pause follow instead of fighting it.
                // The left edge is excluded because that is the panel's own
                // resize handle.
                let body = area.shrink2(Vec2::new(6.0, 0.0));
                let over_body = ui.rect_contains_pointer(body);
                let wheel = ui.input(|input| input.smooth_scroll_delta.y.abs() > 0.5);
                let dragging = ui.ctx().dragged_id().is_some();
                manual_scroll = over_body && (wheel || dragging);

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
                    if is_current && follow && !manual_scroll {
                        response.scroll_to_me(Some(Align::Center));
                    }
                }
                ui.add_space(120.0);
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
                // Look-ahead is over: resume centring. The user's own 跟随
                // switch is deliberately left alone here — only the switch (or
                // the resume pill) may turn following back on.
                self.manual_scroll_at = None;
            }
        }

        // The "follow paused" pill floats over the bottom of the body.
        if !self.follow_lyrics || self.manual_scroll_at.is_some() {
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
        crate::lyrics::LyricsSource::Lrclib => ("LRCLib", true),
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

#[cfg(test)]
mod tests {
    //! The lyrics panel is hand-painted, so these drive the real `App` inside a
    //! real egui pass and inspect the state and the painted output afterwards —
    //! the only way to catch "the switch looks fine but does nothing".

    use super::*;
    use crate::config::Config;
    use crate::lyrics::{LyricLine, Lyrics, LyricsSource};
    use egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2};

    /// The 跟随 and 翻译 pills, in screen coordinates.
    const FOLLOW_PILL: Pos2 = Pos2::new(1160.0, 86.0);
    const TRANSLATE_PILL: Pos2 = Pos2::new(1230.0, 86.0);
    /// Somewhere over the history list, well clear of the lyrics panel.
    const HISTORY_PANE: Pos2 = Pos2::new(400.0, 400.0);
    /// Over the lyric body.
    const LYRIC_BODY: Pos2 = Pos2::new(1086.0, 300.0);

    fn raw(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0))),
            events,
            ..Default::default()
        }
    }

    /// One frame of the whole window, exactly as `App::ui` composes it.
    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<Event>) -> egui::FullOutput {
        let theme = app.theme;
        let mut output = ctx.run_ui(raw(events), |ui| {
            egui::Panel::top("top_bar")
                .exact_size(crate::ui::theme::TOP_BAR_H)
                .frame(crate::ui::frames::top_bar())
                .show(ui, |ui| app.ui_top_bar(ui));
            egui::Panel::bottom("status_bar")
                .exact_size(crate::ui::theme::STATUS_BAR_H)
                .frame(crate::ui::frames::status_bar())
                .show(ui, |ui| app.ui_status_bar(ui));
            egui::Panel::bottom("player_bar")
                .frame(crate::ui::frames::player_bar(&theme))
                .show(ui, |ui| app.ui_player_bar(ui));
            egui::Panel::right("lyrics_panel")
                .resizable(true)
                .default_size(388.0)
                .min_size(260.0)
                .max_size(620.0)
                .frame(crate::ui::frames::lyrics(&theme))
                .show(ui, |ui| app.ui_lyrics_panel(ui));
            egui::CentralPanel::default()
                .frame(crate::ui::frames::list_pane())
                .show(ui, |ui| app.ui_central(ui));
        });
        output.textures_delta.clear();
        output
    }

    fn click(ctx: &egui::Context, app: &mut App, pos: Pos2) {
        frame(ctx, app, vec![Event::PointerMoved(pos)]);
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(ctx, app, vec![Event::PointerMoved(pos)]);
    }

    /// One trackpad scroll sample under the pointer, as the OS reports it.
    fn wheel(pos: Pos2, dy: f32) -> Vec<Event> {
        wheel_phase(pos, dy, egui::TouchPhase::Move)
    }

    /// The end of a trackpad gesture; egui clears its scroll smoothing here.
    fn wheel_end(pos: Pos2) -> Vec<Event> {
        wheel_phase(pos, 0.0, egui::TouchPhase::End)
    }

    fn wheel_phase(pos: Pos2, dy: f32, phase: egui::TouchPhase) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: Vec2::new(0.0, dy),
                modifiers: Modifiers::NONE,
                phase,
            },
        ]
    }

    /// Scrolls, then lets egui's smoothing settle so later frames are quiet.
    fn scroll(ctx: &egui::Context, app: &mut App, pos: Pos2, notches: usize) {
        for _ in 0..notches {
            frame(ctx, app, wheel(pos, -6.0));
        }
        for _ in 0..3 {
            frame(ctx, app, vec![Event::PointerMoved(pos)]);
        }
        frame(ctx, app, wheel_end(pos));
    }

    /// Every string egui actually painted this frame.
    fn painted_text(output: &egui::FullOutput) -> Vec<String> {
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
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn painted(output: &egui::FullOutput, needle: &str) -> bool {
        painted_text(output)
            .iter()
            .any(|text| text.contains(needle))
    }

    fn app_with_lyrics(translated: bool) -> (App, egui::Context) {
        std::env::set_var("HOME", "/tmp/listenbli-lyrics-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-lyrics-tests");
        let mut app = App::new_for_tests(Config::default());
        app.lyrics = Lyrics {
            lines: vec![
                LyricLine {
                    time: Duration::ZERO,
                    text: "first line".to_owned(),
                    translation: translated.then(|| "第一行译文".to_owned()),
                },
                LyricLine {
                    time: Duration::from_secs(30),
                    text: "second line".to_owned(),
                    translation: None,
                },
                LyricLine {
                    time: Duration::from_secs(60),
                    text: "third line".to_owned(),
                    translation: None,
                },
            ],
            source: LyricsSource::Netease,
        };
        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![]);
        }
        (app, ctx)
    }

    /// Enough lyrics that the body scrolls, so there is a scroll bar to grab.
    fn app_with_long_lyrics() -> (App, egui::Context) {
        let (mut app, ctx) = app_with_lyrics(true);
        app.lyrics.lines = (0..40)
            .map(|index| LyricLine {
                time: Duration::from_secs(index * 5),
                text: format!("第 {index} 行歌词"),
                translation: None,
            })
            .collect();
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![]);
        }
        (app, ctx)
    }

    /// Dragging the scroll bar is a manual scroll too: it must pause follow
    /// rather than be yanked back by the auto-centring every frame.
    #[test]
    fn dragging_the_lyric_scroll_bar_pauses_follow() {
        let (mut app, ctx) = app_with_long_lyrics();
        assert!(app.follow_lyrics);

        let press = |pos: Pos2| {
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]
        };
        let release = |pos: Pos2| {
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ]
        };

        // The bar hugs the right edge of the lyric body; find it.
        let mut grabbed = None;
        'scan: for x in (1250..1280).rev() {
            let from = Pos2::new(x as f32, 320.0);
            let to = Pos2::new(x as f32, 220.0);
            app.manual_scroll_at = None;
            frame(&ctx, &mut app, vec![Event::PointerMoved(from)]);
            frame(&ctx, &mut app, press(from));
            frame(&ctx, &mut app, vec![Event::PointerMoved(to)]);
            let paused = app.manual_scroll_at.is_some();
            frame(&ctx, &mut app, release(to));
            frame(&ctx, &mut app, vec![]);
            if paused {
                grabbed = Some(x);
                break 'scan;
            }
        }

        let x = grabbed.expect("no drag reached the lyric scroll bar (tried x = 1250..1280)");
        println!("scroll bar grab hit x = {x}");
        assert!(
            x >= 1260,
            "the drag should land on the bar at the right edge, not on a lyric line (x = {x})"
        );
        assert!(app.follow_lyrics, "a pause must not switch 跟随 off");
    }

    /// Clicking a lyric line still seeks, and is not mistaken for a scroll.
    #[test]
    fn clicking_a_lyric_line_seeks_without_pausing_follow() {
        let (mut app, ctx) = app_with_long_lyrics();
        // An engine seek without a loaded stream is silent, so dropping the
        // engine is what makes the click observable: every seek then reports
        // itself through the status line.
        app.engine = None;

        let mut seeked = Vec::new();
        for y in (150..620).step_by(8) {
            let pos = Pos2::new(1100.0, y as f32);
            app.status = None;
            click(&ctx, &mut app, pos);
            assert!(
                app.manual_scroll_at.is_none(),
                "a click is not a manual scroll (y = {y})"
            );
            if app.status.is_some() {
                seeked.push(y);
            }
        }

        println!("点击歌词行命中的 y: {seeked:?}");
        assert!(
            seeked.len() > 5,
            "clicking a lyric line should seek; only {} of the probed rows reacted",
            seeked.len()
        );
    }

    /// Both pills are hit-testable: each one flips its own switch.
    #[test]
    fn both_toggles_respond_to_clicks() {
        let (mut app, ctx) = app_with_lyrics(true);

        app.show_translation = true;
        app.follow_lyrics = true;
        click(&ctx, &mut app, FOLLOW_PILL);
        assert!(!app.follow_lyrics, "跟随 should switch off");
        assert!(app.show_translation, "跟随 must not touch 翻译");

        app.show_translation = true;
        app.follow_lyrics = true;
        click(&ctx, &mut app, TRANSLATE_PILL);
        assert!(!app.show_translation, "翻译 should switch off");
        assert!(app.follow_lyrics, "翻译 must not touch 跟随");
    }

    /// 翻译 really adds and removes the translated line.
    #[test]
    fn translation_toggle_changes_the_rendered_text() {
        let (mut app, ctx) = app_with_lyrics(true);

        app.show_translation = true;
        let shown = frame(&ctx, &mut app, vec![]);
        assert!(
            painted(&shown, "第一行译文"),
            "the translation should be painted while the switch is on"
        );

        app.show_translation = false;
        let hidden = frame(&ctx, &mut app, vec![]);
        assert!(
            !painted(&hidden, "第一行译文"),
            "the translation must disappear when the switch is off"
        );
    }

    /// With no translation in the lyric data the switch cannot change the text,
    /// so the click has to say so rather than look broken.
    #[test]
    fn a_lyric_without_translation_says_so() {
        let (mut app, ctx) = app_with_lyrics(false);
        app.show_translation = true;

        click(&ctx, &mut app, TRANSLATE_PILL);

        let status = app.status.as_ref().expect("a click must report itself");
        assert!(
            status.text.contains("译文"),
            "unexpected status: {}",
            status.text
        );
    }

    /// Scrolling the history list is not a lyric scroll.
    #[test]
    fn scrolling_the_history_list_leaves_follow_alone() {
        let (mut app, ctx) = app_with_lyrics(true);
        scroll(&ctx, &mut app, HISTORY_PANE, 8);
        assert!(
            app.manual_scroll_at.is_none(),
            "a scroll over the history list must not pause lyric follow"
        );
        assert!(app.follow_lyrics, "跟随 must stay on");

        let output = frame(&ctx, &mut app, vec![]);
        assert!(
            !painted(&output, "跟随已暂停"),
            "the resume pill must not appear for a scroll outside the lyrics"
        );
    }

    /// Scrolling the lyric body itself pauses follow, then resumes by itself.
    #[test]
    fn a_lyric_scroll_pauses_follow_and_then_resumes() {
        let (mut app, ctx) = app_with_lyrics(true);

        scroll(&ctx, &mut app, LYRIC_BODY, 4);
        assert!(
            app.manual_scroll_at.is_some(),
            "scrolling the lyric body should pause follow"
        );
        assert!(app.follow_lyrics, "a pause must not switch 跟随 off");
        let output = frame(&ctx, &mut app, vec![Event::PointerMoved(LYRIC_BODY)]);
        assert!(
            painted(&output, "跟随已暂停"),
            "the resume pill should offer a way back"
        );

        std::thread::sleep(Duration::from_millis(3_200));
        frame(&ctx, &mut app, vec![Event::PointerMoved(LYRIC_BODY)]);
        assert!(
            app.manual_scroll_at.is_none(),
            "follow should resume once the look-ahead window closes"
        );
    }

    /// The switch the user flipped is the last word: it may not turn itself back
    /// on, however much the reader scrolls.
    #[test]
    fn follow_stays_off_until_the_user_says_otherwise() {
        let (mut app, ctx) = app_with_lyrics(true);
        click(&ctx, &mut app, FOLLOW_PILL);
        assert!(!app.follow_lyrics);

        scroll(&ctx, &mut app, LYRIC_BODY, 8);
        std::thread::sleep(Duration::from_millis(3_200));
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![Event::PointerMoved(LYRIC_BODY)]);
        }
        assert!(
            !app.follow_lyrics,
            "跟随 was switched off by hand and must stay off"
        );

        let output = frame(&ctx, &mut app, vec![]);
        assert!(
            painted(&output, "跟随已暂停"),
            "the resume pill should still be offered"
        );
    }
}

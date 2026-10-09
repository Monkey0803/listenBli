//! The settings sheet: quality, lyrics, appearance, fonts and the config file.

use egui::{
    Align, Align2, Color32, Id, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};

use super::icons;
use super::theme::{self as t, ACCENTS};
use super::widgets;
use crate::app::App;
use crate::platform;

impl App {
    pub(crate) fn ui_settings_sheet(&mut self, ctx: &egui::Context) {
        let accent = self.theme.accent;
        let screen = ctx.viewport_rect();
        let mut close = false;

        let modal = egui::Modal::new(Id::new("settings-sheet"))
            .area(egui::Area::new(Id::new("settings-area")).anchor(Align2::RIGHT_TOP, Vec2::ZERO))
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_2)
                    .corner_radius(0.0)
                    .inner_margin(Margin::ZERO)
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .backdrop_color(Color32::from_black_alpha(128))
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.set_min_height(screen.height());

                // -- header --------------------------------------------------
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    // Painted, so the gear shares the title's ink centre rather
                    // than the font's ascent/descent box.
                    let font = t::ui_font(15.0);
                    let width = 23.0 + widgets::measure(ui, "设置", &font).x;
                    let (row, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
                    let painter = ui.painter().clone();
                    icons::settings(
                        &painter,
                        Rect::from_min_size(
                            Pos2::new(row.left(), row.center().y - 8.5),
                            Vec2::splat(17.0),
                        ),
                        accent.accent,
                    );
                    widgets::paint_label(
                        &painter,
                        Pos2::new(row.left() + 23.0, row.center().y),
                        Align2::LEFT_CENTER,
                        "设置",
                        font,
                        t::FG,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        if widgets::icon_button(ui, icons::close, 32.0, false, accent, "关闭设置")
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                ui.add_space(13.0);
                super::hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);

                egui::ScrollArea::vertical()
                    .max_height((screen.height() - 70.0).max(200.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                ui.set_max_width(360.0);
                                self.quality_group(ui, accent);
                                self.lyrics_group(ui, accent);
                                self.appearance_group(ui, accent);
                                self.font_group(ui, accent);
                                self.config_group(ui, accent);
                                ui.add_space(24.0);
                            });
                        });
                    });
            });

        if modal.should_close() {
            close = true;
        }
        if close {
            self.settings_open = false;
        }
    }

    fn quality_group(&mut self, ui: &mut Ui, accent: t::Accent) {
        group(ui, icons::spark, "音质", accent, |ui| {
            let logged_in = self.user.is_some();
            let vip = self.user.as_ref().is_some_and(|u| u.is_vip());
            let mut prefer_flac = self.config_value(|c| c.prefer_flac);
            field_with_switch(
                ui,
                "无损优先",
                if logged_in && vip {
                    "优先请求 FLAC 无损流；仅部分投稿提供，失败时自动回落到 192K。"
                } else if logged_in {
                    "当前账号不是大会员，FLAC 流不可见，仍会请求 192K AAC-LC。"
                } else {
                    "需要大会员账号。当前未登录，只会请求 192K AAC-LC。"
                },
                &mut prefer_flac,
                accent,
            );
            if prefer_flac != self.config_value(|c| c.prefer_flac) {
                self.update_config(|config| config.prefer_flac = prefer_flac);
                self.set_status(
                    if prefer_flac {
                        "已开启无损优先"
                    } else {
                        "已关闭无损优先"
                    },
                    false,
                );
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("当前输出").size(12.5).color(t::FG));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let badge = widgets::Badge::quality(self.quality);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(90.0, 20.0), Sense::hover());
                    widgets::paint_badge(ui, rect, &badge);
                });
            });
        });
    }

    fn lyrics_group(&mut self, ui: &mut Ui, accent: t::Accent) {
        group(ui, icons::lyrics, "歌词", accent, |ui| {
            let mut show_translation = self.show_translation;
            if field_with_switch(
                ui,
                "优先显示翻译",
                "有翻译时在原句下方以蓝色小字显示。",
                &mut show_translation,
                accent,
            )
            .changed()
            {
                self.show_translation = show_translation;
                self.update_config(|config| config.prefer_translation = show_translation);
            }

            ui.add_space(6.0);
            ui.label(RichText::new("来源优先级").size(12.5).color(t::FG));
            ui.add_space(9.0);
            order_row(ui, "01", "B 站 CC 字幕", Some("最准"));
            ui.add_space(7.0);
            order_row(ui, "02", "网易云音乐", Some("含翻译"));
            ui.add_space(7.0);
            order_row_muted(ui, "03", "无歌词（占位提示）");
        });
    }

    fn appearance_group(&mut self, ui: &mut Ui, accent: t::Accent) {
        group(ui, icons::target, "外观", accent, |ui| {
            let current = self.theme.accent;
            ui.horizontal(|ui| {
                ui.label(RichText::new("强调色").size(12.5).color(t::FG));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for preset in ACCENTS.iter().rev() {
                        if widgets::swatch(ui, *preset, preset.id == current.id).clicked() {
                            self.update_config(|config| config.accent = preset.id.to_owned());
                            self.theme.accent = *preset;
                        }
                        ui.add_space(8.0);
                    }
                });
            });

            ui.add_space(10.0);

            let mut size = self.theme.lyric_size;
            ui.horizontal(|ui| {
                ui.label(RichText::new("歌词字号").size(12.5).color(t::FG));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let response = widgets::range(ui, &mut size, 14.0..=20.0, 128.0, accent);
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!("{size:.1}px"))
                            .font(t::mono_font(11.0))
                            .color(t::FG_4),
                    );
                    if response.drag_stopped() || response.lost_focus() {
                        self.update_config(|config| config.lyric_size = size);
                    }
                });
            });
            if (size - self.theme.lyric_size).abs() > f32::EPSILON {
                // Live preview: apply in memory, persist once the drag ends.
                self.set_config_in_memory(|config| config.lyric_size = size);
            }

            ui.add_space(10.0);

            let mut density = self.config_value(|c| c.density.clone());
            ui.horizontal(|ui| {
                ui.label(RichText::new("列表密度").size(12.5).color(t::FG));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let options = [("comfortable", "舒适"), ("compact", "紧凑")];
                    if widgets::segmented(ui, &options, &mut density) {
                        self.update_config(|config| config.density = density.clone());
                    }
                });
            });
        });
    }

    fn font_group(&mut self, ui: &mut Ui, accent: t::Accent) {
        group(ui, icons::translate, "中文字体", accent, |ui| {
            let mut choice = self.config_value(|c| c.cjk_font.clone());
            let mut picked = None;
            ui.horizontal_wrapped(|ui| {
                for (id, name) in platform::CJK_FONT_CHOICES {
                    if widgets::chip(ui, name, None, None, choice == id, accent).clicked() {
                        picked = Some(id);
                    }
                    ui.add_space(2.0);
                }
            });
            if let Some(id) = picked {
                choice = id.to_owned();
                self.update_config(|config| config.cjk_font = choice.clone());
            }

            ui.add_space(10.0);
            ui.label(
                RichText::new("自动探测失败时界面中文会显示为方块，此时在下面填入字体绝对路径。")
                    .size(11.0)
                    .color(t::FG_3),
            );
            ui.add_space(9.0);

            let mut path = self.cjk_path_input.clone();
            let button = 32.0;
            let gap = 8.0;
            // 10px of padding on each side, so the text lines up with the
            // design's `.input`.
            let field = (ui.available_width() - button - gap - 20.0).max(80.0);
            ui.horizontal(|ui| {
                let input = egui::Frame::NONE
                    .fill(Color32::from_black_alpha(102))
                    .corner_radius(t::R_SM)
                    .stroke(Stroke::new(1.0, t::LINE))
                    .inner_margin(Margin {
                        left: 10,
                        right: 10,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        let font = t::mono_font(11.5);
                        let offset = widgets::field_text_offset(ui.painter(), &font);
                        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                            widgets::field_galley(ui, buf.as_str(), &font, t::FG, offset, wrap)
                        };
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut path)
                                .frame(egui::Frame::NONE)
                                .margin(Margin::ZERO)
                                .min_size(Vec2::new(field, 32.0))
                                .vertical_align(Align::Center)
                                .layouter(&mut layouter)
                                .font(font.clone())
                                .text_color(t::FG),
                        );
                        if path.is_empty() {
                            widgets::paint_label(
                                ui.painter(),
                                response.rect.left_center(),
                                Align2::LEFT_CENTER,
                                platform::cjk_font_hint(),
                                font,
                                t::FG_4,
                            );
                        }
                        response
                    });
                let response = input.inner;
                if response.has_focus() {
                    let painter = ui.painter().clone();
                    painter.rect_stroke(
                        input.response.rect,
                        t::R_SM,
                        Stroke::new(1.0, t::fade(accent.accent, 0.5)),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_stroke(
                        input.response.rect.expand(3.0),
                        11.0,
                        Stroke::new(3.0, accent.dim()),
                        egui::StrokeKind::Outside,
                    );
                }

                ui.add_space(gap);
                if widgets::icon_button(ui, icons::copy, button, false, accent, "复制路径")
                    .clicked()
                {
                    ui.ctx().copy_text(path.clone());
                    self.notify("已复制到剪贴板");
                }
                if response.changed() {
                    // Hold the edit locally: applying it per keystroke would
                    // rebuild the whole font atlas on every character.
                    self.cjk_path_input = path.clone();
                }
                if response.lost_focus() {
                    let saved = self.cjk_path_input.clone();
                    self.update_config(|config| {
                        config.cjk_font_path = (!saved.trim().is_empty())
                            .then(|| std::path::PathBuf::from(saved.trim()));
                    });
                }
            });

            // The files actually found on this machine, so a broken setting is
            // obvious rather than mysterious.
            ui.add_space(8.0);
            let resolved = platform::resolve_cjk_font(
                &choice,
                self.config_value(|c| c.cjk_font_path.clone()).as_deref(),
            );
            let text = match resolved {
                Some(path) => format!("当前使用：{}", path.display()),
                None => "未找到可用字体".to_owned(),
            };
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
            widgets::clipped_line(ui, row, &text, t::ui_font(10.5), t::FG_4, false);
        });
    }

    fn config_group(&mut self, ui: &mut Ui, accent: t::Accent) {
        group(ui, icons::folder, "配置文件", accent, |ui| {
            // Laid out by hand: the path is longer than the sheet is wide, so
            // it has to be elided before the button gets its slot — otherwise
            // it pushes the button out past the edge of the sheet.
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
            let button = 30.0;
            let button_rect = Rect::from_min_size(
                Pos2::new(row.right() - button, row.center().y - button * 0.5),
                Vec2::splat(button),
            );
            widgets::clipped_line(
                ui,
                Rect::from_min_max(row.min, Pos2::new(button_rect.left() - 10.0, row.bottom())),
                &crate::config::config_path_display(),
                t::mono_font(11.0),
                t::FG_3,
                false,
            );
            if widgets::icon_button_at(
                ui,
                Id::new("copy-config-path"),
                button_rect,
                icons::copy,
                false,
                accent,
                "复制配置路径",
            )
            .clicked()
            {
                ui.ctx().copy_text(crate::config::config_path_display());
                self.notify("已复制到剪贴板");
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "音量、无损开关、歌词偏好与登录凭据都存在这里；Windows 下位于 \
                     %APPDATA%\\listenBli\\config\\。",
                )
                .size(11.0)
                .color(t::FG_3),
            );
        });
    }
}

/// A titled group inside the sheet.
fn group(
    ui: &mut Ui,
    icon: icons::Icon,
    name: &str,
    accent: t::Accent,
    body: impl FnOnce(&mut Ui),
) {
    ui.add_space(16.0);
    // Painted rather than laid out: an icon beside a label has to be centred on
    // the label's *ink*, which egui's own row layout cannot express.
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let painter = ui.painter().clone();
    let galley = painter.layout_no_wrap(name.to_owned(), t::ui_font(11.0), t::FG_3);
    icon(
        &painter,
        Rect::from_min_size(
            Pos2::new(row.left(), row.center().y - 6.5),
            Vec2::splat(13.0),
        ),
        accent.accent,
    );
    widgets::paint_galley_centred(
        &painter,
        Pos2::new(row.left() + 17.0, row.center().y),
        galley,
        t::FG_3,
    );
    ui.add_space(13.0);
    body(ui);
    ui.add_space(16.0);
    super::hline(ui, ui.min_rect().bottom() - 0.5, t::LINE);
}

/// A label + hint on the left, a switch on the right.
fn field_with_switch(
    ui: &mut Ui,
    label: &str,
    hint: &str,
    checked: &mut bool,
    accent: t::Accent,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(label).size(12.5).color(t::FG));
            if !hint.is_empty() {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(hint)
                            .size(11.0)
                            .color(t::FG_3)
                            .line_height(Some(18.0)),
                    )
                    .wrap(),
                );
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            widgets::switch(ui, checked, accent)
        })
        .inner
    })
    .inner
}

fn order_row(ui: &mut Ui, number: &str, label: &str, note: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, t::R_SM, t::white(0.032));
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, t::LINE),
        egui::StrokeKind::Inside,
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + 11.0, rect.center().y),
        Align2::LEFT_CENTER,
        number,
        t::mono_font(10.0),
        t::ACCENTS[0].accent,
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        t::ui_font(12.0),
        t::FG_2,
    );
    if let Some(note) = note {
        widgets::paint_label(
            &painter,
            Pos2::new(rect.right() - 11.0, rect.center().y),
            Align2::RIGHT_CENTER,
            note,
            t::ui_font(10.5),
            t::FG_4,
        );
    }
}

fn order_row_muted(ui: &mut Ui, number: &str, label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, t::R_SM, t::white(0.032));
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, t::LINE),
        egui::StrokeKind::Inside,
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + 11.0, rect.center().y),
        Align2::LEFT_CENTER,
        number,
        t::mono_font(10.0),
        t::FG_4,
    );
    widgets::paint_label(
        &painter,
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        t::ui_font(12.0),
        t::FG_4,
    );
}

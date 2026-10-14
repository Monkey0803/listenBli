//! 来源详情 — which copy is playing, and why.
//!
//! The design keeps the platform out of the library entirely and then gives you
//! exactly one place to ask about it. Everything here is derived from the same
//! preference rule `LocalLibrary::preferred` applies, so what this sheet says and
//! what the player does cannot disagree.

use egui::{Align, Align2, Color32, Id, Layout, Margin, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::api::models::{Source, Track};
use crate::app::{App, CacheInfo, Cached};
use crate::library::{LibraryEntry, LocalLibrary};

use super::icons;
use super::songs;
use super::theme::{self as t};
use super::widgets::{self, Emphasis};

const SHEET_W: f32 = 620.0;
const PAD: f32 = 18.0;
const SHEET_MAX_H: f32 = 540.0;

/// What the sheet's buttons asked for, applied after the modal has closed.
///
/// The body reads the library while it draws, so it cannot also write to it:
/// every press is collected here and acted on afterwards.
enum Ask {
    Close,
    OpenVideo(String),
    /// Make this version the one that plays.
    Prefer(String),
    /// 换掉 on a dead edition: keep the working copy instead, for good.
    ReplaceDead,
}

/// One platform copy of a song, as the sheet needs it.
struct Edition {
    key: String,
    bvid: String,
    source: Source,
    author: String,
    duration: u64,
    bytes: u64,
    state: Cached,
    alive: bool,
    picked: bool,
    pinned: bool,
}

impl App {
    pub(crate) fn ui_source_sheet(&mut self, ctx: &egui::Context) {
        let accent = self.theme.accent;
        let Some(current) = self.current.clone() else {
            self.source_open = false;
            return;
        };
        let key = LocalLibrary::key_of(&current);
        let entry = self.library.find(&key).cloned();
        let pinned = entry.as_ref().and_then(|entry| entry.pinned.clone());

        let cache = |track: &Track| self.cache_info_of(track);
        let alive = |track: &Track| !self.dead.contains(&track.key());
        let picked_key = entry
            .as_ref()
            .and_then(|entry| {
                LocalLibrary::preferred(
                    entry,
                    |track| cache(track).state == Cached::Complete,
                    alive,
                )
            })
            .map(|track| track.key());
        let editions = entry
            .as_ref()
            .map(|entry| editions_of(entry, &cache, &alive, &picked_key))
            .unwrap_or_default();
        let chosen_quality = self.quality.map(|quality| quality.label());

        let mut ask: Option<Ask> = None;
        let response = egui::Modal::new(Id::new("source-sheet"))
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_2)
                    .corner_radius(t::R_MD)
                    .inner_margin(Margin::ZERO)
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .backdrop_color(Color32::from_black_alpha(150))
            .show(ctx, |ui| {
                ui.set_width(SHEET_W);
                self.sheet_head(ui, &current, accent, &mut ask);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(SHEET_MAX_H)
                    .show(ui, |ui| {
                        ui.add_space(16.0);
                        label(ui, "正在播放的这一版");
                        match &picked_key {
                            Some(_) => {
                                if let Some(edition) = editions
                                    .iter()
                                    .find(|edition| Some(&edition.key) == picked_key.as_ref())
                                {
                                    playing_box(ui, edition, chosen_quality, accent, &mut ask);
                                }
                            }
                            None if entry.is_some() => dead_box(ui, editions.len()),
                            None => not_in_library(ui),
                        }

                        ui.add_space(15.0);
                        label(ui, "为什么是这一版");
                        why(ui, &entry, &editions, &picked_key, accent);

                        if !editions.is_empty() {
                            ui.add_space(15.0);
                            label(ui, &format!("这首歌的全部版本 · {}", editions.len()));
                            for edition in &editions {
                                edition_row(ui, edition, picked_key.as_deref(), accent, &mut ask);
                            }
                        }
                        ui.add_space(16.0);
                    });

                self.sheet_foot(
                    ui,
                    &editions,
                    pinned.as_deref(),
                    &picked_key,
                    accent,
                    &mut ask,
                );
            });

        if response.should_close() {
            ask = Some(Ask::Close);
        }
        self.apply_ask(ask, &key);
    }

    fn cache_info_of(&self, track: &Track) -> CacheInfo {
        self.cache_index
            .get(&LocalLibrary::key_of(track))
            .copied()
            .unwrap_or(CacheInfo {
                state: Cached::None,
                bytes: 0,
            })
    }

    fn sheet_head(&self, ui: &mut Ui, current: &Track, accent: t::Accent, ask: &mut Option<Ask>) {
        let (head, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, 62.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.hline(
            head.x_range(),
            head.bottom() - 0.5,
            Stroke::new(1.0, t::LINE),
        );
        let cover = Rect::from_min_size(
            Pos2::new(head.left() + PAD, head.top() + 14.0),
            Vec2::splat(34.0),
        );
        widgets::paint_cover(
            &painter,
            cover,
            Some(current),
            self.covers.get(&current.key()),
        );
        widgets::label_at(
            ui,
            Pos2::new(cover.right() + 12.0, cover.top() + 8.0),
            Align2::LEFT_CENTER,
            &current.title,
            t::ui_font(15.0),
            t::FG,
        );
        widgets::label_at(
            ui,
            Pos2::new(cover.right() + 12.0, cover.top() + 26.0),
            Align2::LEFT_CENTER,
            &songs::subtitle(current),
            t::mono_font(10.0),
            t::FG_4,
        );
        let close = Rect::from_min_size(
            Pos2::new(head.right() - PAD - 26.0, head.top() + 18.0),
            Vec2::splat(26.0),
        );
        if widgets::icon_button_at(
            ui,
            Id::new("source-sheet-close"),
            close,
            icons::close,
            false,
            accent,
            "关闭",
        )
        .clicked()
        {
            *ask = Some(Ask::Close);
        }
    }

    fn sheet_foot(
        &self,
        ui: &mut Ui,
        editions: &[Edition],
        pinned: Option<&str>,
        picked: &Option<String>,
        accent: t::Accent,
        ask: &mut Option<Ask>,
    ) {
        let (foot, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, 48.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.hline(foot.x_range(), foot.top() + 0.5, Stroke::new(1.0, t::LINE));

        let others = editions.iter().filter(|edition| !edition.picked).count();
        widgets::label_at(
            ui,
            Pos2::new(foot.left() + PAD, foot.center().y),
            Align2::LEFT_CENTER,
            &format!("{others} 个其它版本"),
            t::mono_font(10.0),
            t::FG_4,
        );

        let actions = Rect::from_min_max(
            Pos2::new(foot.right() - PAD - 220.0, foot.center().y - 13.0),
            Pos2::new(foot.right() - PAD, foot.center().y + 13.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(actions)
                .layout(Layout::right_to_left(Align::Center)),
        );
        if widgets::console_button(&mut child, "关闭", None, Emphasis::Ghost, accent).clicked() {
            *ask = Some(Ask::Close);
        }
        child.add_space(8.0);
        // 固定使用这一版 pins whichever copy the rules chose, which is what stops
        // the rules from moving it. With nothing to pin there is no button: a
        // button that cannot do anything is worse than no button.
        if let Some(picked) = picked {
            let already = pinned == Some(picked.as_str());
            let response = widgets::console_button(
                &mut child,
                if already {
                    "已固定"
                } else {
                    "固定使用这一版"
                },
                None,
                Emphasis::Primary,
                accent,
            );
            if response.clicked() && !already {
                *ask = Some(Ask::Prefer(picked.clone()));
            }
        }
    }

    /// Acts on whatever the sheet asked for.
    fn apply_ask(&mut self, ask: Option<Ask>, key: &str) {
        match ask {
            Some(Ask::Close) => self.source_open = false,
            Some(Ask::OpenVideo(bvid)) => {
                let url = crate::util::video_url(&bvid);
                if let Err(err) = crate::platform::open_url(&url) {
                    self.set_status(err, true);
                }
            }
            Some(Ask::ReplaceDead) => {
                let working = self
                    .library
                    .find(key)
                    .and_then(|entry| {
                        LocalLibrary::preferred(
                            entry,
                            |track| self.cache_info_of(track).state == Cached::Complete,
                            |track| !self.dead.contains(&track.key()),
                        )
                    })
                    .map(|track| track.key());
                match working {
                    Some(version) => self.apply_ask(Some(Ask::Prefer(version)), key),
                    None => self.set_status("这首歌没有可用的版本了", true),
                }
            }
            Some(Ask::Prefer(version)) => {
                if self.library.pin(key, Some(version.clone())) {
                    self.save_library();
                    let name = self
                        .library
                        .find(key)
                        .map(|entry| {
                            entry
                                .versions
                                .iter()
                                .chain(std::iter::once(&entry.track))
                                .find(|track| track.key() == version)
                                .map(|track| track.source.label())
                                .unwrap_or("这一")
                        })
                        .unwrap_or("这一");
                    self.set_status(format!("已改用 {name} 那一版"), false);
                } else {
                    self.set_status("这一版已经不在库里了", true);
                }
            }
            None => {}
        }
    }
}

/// Every version of a song, in the library's own order.
fn editions_of(
    entry: &LibraryEntry,
    cache: &dyn Fn(&Track) -> CacheInfo,
    alive: &dyn Fn(&Track) -> bool,
    picked: &Option<String>,
) -> Vec<Edition> {
    let versions: Vec<&Track> = if entry.versions.is_empty() {
        vec![&entry.track]
    } else {
        entry.versions.iter().collect()
    };
    versions
        .into_iter()
        .map(|track| {
            let key = track.key();
            let info = cache(track);
            Edition {
                alive: alive(track),
                bvid: track.bvid.clone(),
                source: track.source,
                author: track.author.clone(),
                duration: track.duration,
                bytes: info.bytes,
                state: info.state,
                picked: picked.as_deref() == Some(key.as_str()),
                pinned: entry.pinned.as_deref() == Some(key.as_str()),
                key,
            }
        })
        .collect()
}

/// The full-width row rect every block below insets into.
fn row(ui: &mut Ui, height: f32) -> Rect {
    let (full, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, height), Sense::hover());
    Rect::from_min_size(
        Pos2::new(full.left() + PAD, full.top()),
        Vec2::new(SHEET_W - PAD * 2.0, height),
    )
}

/// An uppercase micro-label above a group of rows.
fn label(ui: &mut Ui, title: &str) {
    let (line, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, 18.0), Sense::hover());
    widgets::engraved_at(
        ui,
        Pos2::new(line.left() + PAD, line.center().y),
        Align2::LEFT_CENTER,
        title,
        9.0,
        t::FG_4,
    );
    ui.add_space(2.0);
}

/// The copy that is playing: source, quality, size, and a way to open it.
fn playing_box(
    ui: &mut Ui,
    edition: &Edition,
    quality: Option<&'static str>,
    accent: t::Accent,
    ask: &mut Option<Ask>,
) {
    let rect = row(ui, 58.0);
    let painter = ui.painter().clone();
    painter.rect_filled(rect, t::R_SM, t::INK_1);
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, t::fade(accent.accent, 0.4)),
        egui::StrokeKind::Inside,
    );
    // The amber edge the design uses to mark the live copy.
    painter.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
        0.0,
        accent.accent,
    );
    t::led(
        &painter,
        Pos2::new(rect.left() + 16.0, rect.center().y),
        t::Led::On,
        accent,
    );

    let name = if edition.alive {
        edition.source.label().to_owned()
    } else {
        format!("{}（已失效）", edition.source.label())
    };
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 30.0, rect.top() + 18.0),
        Align2::LEFT_CENTER,
        &name,
        t::ui_font(12.5),
        t::FG,
    );
    let detail = match (quality, edition.state) {
        (Some(quality), Cached::Complete) => format!(
            "{quality} · 已下载 {}",
            crate::net::human_bytes(edition.bytes)
        ),
        (Some(quality), _) => format!("{quality} · 在线播放"),
        (None, Cached::Complete) => {
            format!("已下载 {}", crate::net::human_bytes(edition.bytes))
        }
        (None, _) => "在线播放".to_owned(),
    };
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 30.0, rect.top() + 39.0),
        Align2::LEFT_CENTER,
        &detail,
        t::mono_font(10.0),
        t::FG_4,
    );

    let button = Rect::from_min_size(
        Pos2::new(rect.right() - 14.0 - 116.0, rect.center().y - 13.0),
        Vec2::new(116.0, 26.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(button)
            .layout(Layout::left_to_right(Align::Center)),
    );
    if widgets::console_button(
        &mut child,
        "打开原视频",
        Some(icons::external),
        Emphasis::Ghost,
        accent,
    )
    .clicked()
    {
        *ask = Some(Ask::OpenVideo(edition.bvid.clone()));
    }
}

/// Everything is gone: the song stays in the library, and this says so.
fn dead_box(ui: &mut Ui, count: usize) {
    let rect = row(ui, 58.0);
    let painter = ui.painter().clone();
    painter.rect_filled(
        rect,
        t::R_SM,
        Color32::from_rgba_unmultiplied(0xf2, 0x63, 0x5f, 20),
    );
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, t::fade(t::RED, 0.4)),
        egui::StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
        0.0,
        t::RED,
    );
    icons::close(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 15.0, rect.top() + 15.0),
            Vec2::splat(14.0),
        ),
        t::RED,
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 38.0, rect.top() + 18.0),
        Align2::LEFT_CENTER,
        &format!("{count} 个版本都已失效"),
        t::ui_font(12.5),
        t::FG,
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 38.0, rect.top() + 39.0),
        Align2::LEFT_CENTER,
        "库里保留着它，没有悄悄删掉",
        t::mono_font(10.0),
        t::FG_4,
    );
}

/// The song is playing but is not in the library yet, so there is nothing to
/// compare — and saying that is better than an empty list.
fn not_in_library(ui: &mut Ui) {
    let rect = row(ui, 42.0);
    let painter = ui.painter().clone();
    painter.rect_filled(rect, t::R_SM, t::INK_1);
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(1.0, t::LINE),
        egui::StrokeKind::Inside,
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        "这首歌还不在库里，收藏之后这里会列出所有平台副本",
        t::ui_font(11.5),
        t::FG_3,
    );
}

/// The rule, spelled out for this song.
fn why(
    ui: &mut Ui,
    entry: &Option<LibraryEntry>,
    editions: &[Edition],
    picked: &Option<String>,
    accent: t::Accent,
) {
    let text = match (entry, picked) {
        (Some(_), None) => {
            "这首歌的所有版本都取不到了。库里保留着它，而不是把它悄悄删掉 —— 如果你在别处重新找到，可以直接换上。"
                .to_owned()
        }
        (Some(entry), Some(picked)) => {
            if entry.pinned.as_deref() == Some(picked.as_str()) {
                "你固定了这一版，所以音质和可用性规则不会再换掉它。改用别版就会恢复自动挑选。"
                    .to_owned()
            } else if editions
                .iter()
                .any(|edition| edition.picked && edition.key == entry.track.key())
            {
                "按 音质 › 已下载 › 可用性 排序，这一版最靠前。这个优先级可以在设置里改。"
                    .to_owned()
            } else {
                "本来首选的是另一版，但它已经失效。库里有可用的一版，所以自动换成了它 —— 这首歌没有变成不能播。"
                    .to_owned()
            }
        }
        (None, _) => "这首歌还不在库里，所以没有可比较的版本。".to_owned(),
    };

    let font = t::ui_font(11.5);
    let width = SHEET_W - PAD * 2.0 - 24.0;
    let galley = ui.painter().layout(text, font, t::FG_3, width);
    let height = galley.size().y.max(16.0);
    let (full, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, height), Sense::hover());
    let left = full.left() + PAD;
    icons::filter(
        ui.painter(),
        Rect::from_min_size(Pos2::new(left, full.top() + 2.0), Vec2::splat(13.0)),
        t::FG_4,
    );
    ui.painter()
        .galley(Pos2::new(left + 22.0, full.top()), galley, t::FG_3);
    let _ = accent;
}

/// One version in the list, with the control that switches to it.
fn edition_row(
    ui: &mut Ui,
    edition: &Edition,
    picked: Option<&str>,
    accent: t::Accent,
    ask: &mut Option<Ask>,
) {
    let rect = row(ui, 48.0);
    let painter = ui.painter().clone();
    if edition.picked {
        painter.rect_filled(rect, t::R_SM, t::fade(accent.accent, 0.08));
    }
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(
            1.0,
            if edition.picked {
                t::fade(accent.accent, 0.4)
            } else {
                t::LINE
            },
        ),
        egui::StrokeKind::Inside,
    );

    let led = if edition.picked {
        t::Led::On
    } else if !edition.alive {
        t::Led::Bad
    } else {
        t::Led::Ok
    };
    t::led(
        &painter,
        Pos2::new(rect.left() + 15.0, rect.center().y),
        led,
        accent,
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 30.0, rect.top() + 16.0),
        Align2::LEFT_CENTER,
        edition.source.label(),
        t::ui_font(12.0),
        if edition.alive { t::FG } else { t::FG_3 },
    );
    let note = if !edition.alive {
        "这一版已经失效".to_owned()
    } else {
        format!(
            "{} · {}",
            if edition.author.is_empty() {
                "未知上传者"
            } else {
                edition.author.as_str()
            },
            crate::util::format_duration(edition.duration)
        )
    };
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 30.0, rect.top() + 33.0),
        Align2::LEFT_CENTER,
        &note,
        t::mono_font(9.5),
        t::FG_4,
    );

    let state = match (edition.state, edition.alive) {
        (_, false) => "失效".to_owned(),
        (Cached::Complete, _) => format!("已下载 {}", crate::net::human_bytes(edition.bytes)),
        (Cached::Partial, _) => "仅前半".to_owned(),
        (Cached::None, _) => "在线".to_owned(),
    };
    widgets::label_at(
        ui,
        Pos2::new(rect.right() - 14.0 - 128.0, rect.center().y),
        Align2::RIGHT_CENTER,
        &state,
        t::mono_font(10.0),
        t::FG_3,
    );

    let control = Rect::from_min_size(
        Pos2::new(rect.right() - 14.0 - 112.0, rect.center().y - 12.0),
        Vec2::new(112.0, 24.0),
    );
    if edition.picked {
        // Nothing to press: this is the copy in use, and the amber edge already
        // says so. 已固定 distinguishes "the rules chose it" from "you did".
        widgets::paint_label(
            &painter,
            Pos2::new(control.right(), control.center().y),
            Align2::RIGHT_CENTER,
            if edition.pinned {
                "已固定"
            } else {
                "正在用"
            },
            t::mono_font(10.0),
            accent.accent,
        );
        return;
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(control)
            .layout(Layout::right_to_left(Align::Center)),
    );
    // A dead version offers nothing but 换掉, and even that does not switch to it:
    // it is the same "find me another" the rest of the library already does.
    let label = if edition.alive {
        "改用这一版"
    } else {
        "换掉"
    };
    // A dead version cannot be switched *to*, so its button means the other thing
    // a person wants: stop offering this one. It pins the copy that works, which
    // is what keeps the broken edition from coming back after a restart — the
    // dead set only lasts a session.
    if widgets::console_button(&mut child, label, None, Emphasis::Ghost, accent).clicked() {
        if edition.alive {
            *ask = Some(Ask::Prefer(edition.key.clone()));
        } else if picked.is_some() {
            *ask = Some(Ask::ReplaceDead);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::AudioQuality;
    use crate::config::Config;
    use egui::{Pos2, RawInput, Rect, Vec2};

    fn track(bvid: &str, source: Source) -> Track {
        Track {
            bvid: bvid.to_owned(),
            source,
            aid: 1,
            cid: 2,
            title: "晴天".to_owned(),
            author: if source == Source::Bilibili {
                "杰威尔音乐".to_owned()
            } else {
                "JVR Music".to_owned()
            },
            duration: 269,
            cover: None,
            album: Some("叶惠美".to_owned()),
            artist: Some("周杰伦".to_owned()),
        }
    }

    fn raw() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
            ..Default::default()
        }
    }

    fn texts(output: &egui::FullOutput) -> Vec<String> {
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
        for shape in &output.shapes {
            walk(&shape.shape, &mut out);
        }
        out
    }

    /// An app playing a song that exists on both platforms.
    fn app_with_two_editions() -> (App, egui::Context, Track) {
        crate::platform::serialise_home_tests();
        std::env::set_var("HOME", "/tmp/listenbli-source-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-source-tests");
        let mut app = App::new_for_tests(Config::default());
        // `HOME` is process-global and these tests write to the library file, so
        // one test's pin would otherwise be another test's starting state.
        app.library = LocalLibrary::default();
        let bili = track("BV1sourcesheet", Source::Bilibili);
        let youtube = track("yt-sourcesheet", Source::Youtube);
        app.library_mut().add(bili.clone());
        app.library_mut().add(youtube);
        app.show_as_playing(bili.clone(), Some(AudioQuality::Flac));
        app.source_open = true;

        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);
        (app, ctx, bili)
    }

    /// A sheet in a modal is laid out over more than one pass, so read it once it
    /// has stopped being empty.
    fn sheet(app: &mut App, ctx: &egui::Context) -> Vec<String> {
        let mut painted = Vec::new();
        for _ in 0..4 {
            let mut output = ctx.run_ui(raw(), |ui| app.ui_source_sheet(ui.ctx()));
            output.textures_delta.clear();
            painted = texts(&output);
            if !painted.is_empty() {
                break;
            }
        }
        painted
    }

    /// The sheet is the one place the platform shows, so it has to name every
    /// copy and say which one is in use.
    #[test]
    fn the_sheet_names_every_edition_and_the_one_in_use() {
        let (mut app, ctx, _) = app_with_two_editions();
        let painted = sheet(&mut app, &ctx);
        for needle in [
            "正在播放的这一版",
            "为什么是这一版",
            "这首歌的全部版本 · 2",
            "B站",
            "YouTube",
            "正在用",
            "改用这一版",
            "固定使用这一版",
        ] {
            assert!(
                painted.iter().any(|text| text == needle),
                "{needle} should be painted; painted instead: {painted:?}"
            );
        }
    }

    /// When the first choice is gone the sheet must say so, and say what is
    /// playing instead — that is the whole of 已回退.
    #[test]
    fn the_sheet_explains_a_fallback_and_names_the_copy_that_took_over() {
        let (mut app, ctx, bili) = app_with_two_editions();
        app.mark_dead(&bili);
        let painted = sheet(&mut app, &ctx);
        assert!(
            painted
                .iter()
                .any(|text| text.contains("本来首选的是另一版")),
            "the sheet should explain the fallback; painted instead: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text.contains("这一版已经失效")),
            "the dead edition should be labelled as such"
        );
    }

    /// 换掉 on a dead edition is not a no-op: it pins the copy that works, which
    /// is what keeps the broken one from coming back — the dead set only lasts a
    /// session, a pin does not.
    #[test]
    fn replacing_a_dead_edition_pins_the_copy_that_works() {
        let (mut app, _ctx, bili) = app_with_two_editions();
        app.mark_dead(&bili);
        let key = LocalLibrary::key_of(&bili);

        app.apply_ask(Some(Ask::ReplaceDead), &key);

        let pinned = app
            .library
            .find(&key)
            .expect("still there")
            .pinned
            .clone()
            .expect("a working copy was pinned");
        assert_ne!(
            pinned,
            bili.key(),
            "the dead edition is not what got pinned"
        );
        assert!(!app.dead.contains(&pinned), "and what is pinned plays");
        let status = app.status.as_ref().expect("the change is reported");
        assert!(
            status.text.contains("已改用"),
            "unexpected status: {}",
            status.text
        );
    }

    /// With nothing playable there is nothing to pin, and the sheet says so
    /// rather than pinning a copy that is known to be broken.
    #[test]
    fn replacing_when_nothing_works_reports_that() {
        let (mut app, _ctx, bili) = app_with_two_editions();
        let key = LocalLibrary::key_of(&bili);
        let youtube = app
            .library
            .find(&key)
            .expect("there")
            .versions
            .iter()
            .find(|track| track.source == Source::Youtube)
            .cloned()
            .expect("the other platform's copy");
        app.mark_dead(&bili);
        app.mark_dead(&youtube);

        app.apply_ask(Some(Ask::ReplaceDead), &key);

        let status = app.status.as_ref().expect("reported");
        assert!(status.is_error, "nothing worked, so it is an error");
        assert!(
            app.library.find(&key).expect("there").pinned.is_none(),
            "nothing playable means nothing to pin"
        );
    }

    /// A song that is not in the library yet has nothing to compare, and saying
    /// that is better than an empty list.
    #[test]
    fn a_song_outside_the_library_says_so() {
        crate::platform::serialise_home_tests();
        std::env::set_var("HOME", "/tmp/listenbli-source-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-source-tests");
        let mut app = App::new_for_tests(Config::default());
        app.library = LocalLibrary::default();
        let loose = track("BV1notinthelibrary", Source::Bilibili);
        app.show_as_playing(loose, None);
        app.source_open = true;
        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);

        let painted = sheet(&mut app, &ctx);
        assert!(
            painted.iter().any(|text| text.contains("这首歌还不在库里")),
            "painted instead: {painted:?}"
        );
        assert!(
            !painted.iter().any(|text| text.contains("全部版本")),
            "there are no versions to list for a song outside the library"
        );
    }
}

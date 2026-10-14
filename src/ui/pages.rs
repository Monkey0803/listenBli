//! The six library pages.
//!
//! The old central pane dispatched on `Tab` — one platform's search, favourites
//! and history. A library has no platform: 我的歌 is every source merged, and the
//! platforms only surface on 来源与同步 and behind a song's 来源 button.

use egui::{Align, Align2, Color32, Layout, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::api::models::Source;
use crate::app::{App, Cached, LibraryFilter, QUALITY_TAGS};
use crate::library::{LibraryEntry, LocalLibrary};

use super::icons;
use super::songs::{self, RowAction, SongAction, SongRow, SongState};
use super::theme::{self as t};
use super::widgets::{self, Emphasis};

/// Height of a page header, matching the design's `78px`.
const HEAD_H: f32 = 78.0;

impl App {
    /// The 我的歌 page: every song you have, from every source.
    pub(crate) fn ui_library_page(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let filter = self.library_filter;
        let total = self.library.entries.len();
        let playlists = self.library.playlists.len();
        let downloaded = self
            .library
            .entries
            .iter()
            .filter(|entry| self.cached_state(entry) == Cached::Complete)
            .count();
        let issues = self
            .library
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    self.entry_state(entry),
                    SongState::Gone | SongState::Fallback | SongState::Partial
                )
            })
            .count();

        // ---- header ----------------------------------------------------------
        let (head, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), HEAD_H), Sense::hover());
        let painter = ui.painter().clone();
        painter.hline(
            head.x_range(),
            head.bottom() - 0.5,
            Stroke::new(1.0, t::LINE),
        );

        widgets::label_at_bold(
            ui,
            Pos2::new(head.left() + 16.0, head.top() + 26.0),
            Align2::LEFT_CENTER,
            "我的歌",
            t::ui_font(19.0),
            t::FG,
        );
        let title_w = widgets::measure(ui, "我的歌", t::ui_font(19.0)).x;
        let tag_rect = Rect::from_min_size(
            Pos2::new(head.left() + 16.0 + title_w + 12.0, head.top() + 16.0),
            Vec2::new(96.0, 20.0),
        );
        // "Collected in N places" is a count of real places: this machine, plus one
        // per platform that has actually contributed a collection. A number written
        // into the code would be a claim about somebody's library that nothing
        // checked.
        {
            let mut tag_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(tag_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            widgets::console_tag(
                &mut tag_ui,
                &format!("跨 {} 处收藏", self.collection_places()),
                None,
                false,
                accent,
            );
        }

        let meta = format!(
            "{total} 首 · {playlists} 个歌单 · 已下载 {downloaded} 首 · {issues} 首需要留意"
        );
        widgets::label_at(
            ui,
            Pos2::new(head.left() + 17.0, head.top() + 52.0),
            Align2::LEFT_CENTER,
            &meta,
            t::mono_font(10.5),
            t::FG_4,
        );

        // The filter row sits at the header's right edge, in its own child ui so it
        // lays itself out without disturbing the hand-painted title.
        let seg_w = LibraryFilter::ALL
            .iter()
            .map(|f| widgets::measure(ui, f.label(), t::ui_font(11.5)).x + 22.0)
            .sum::<f32>()
            + 2.0;
        let actions = Rect::from_min_max(
            Pos2::new(head.right() - 16.0 - seg_w - 96.0, head.top() + 12.0),
            Pos2::new(head.right() - 16.0, head.top() + 38.0),
        );
        {
            let mut actions_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(actions)
                    .layout(Layout::right_to_left(Align::Center)),
            );
            widgets::console_button(&mut actions_ui, "播放", None, Emphasis::Primary, accent);
            actions_ui.add_space(8.0);
            let selected = LibraryFilter::ALL
                .iter()
                .position(|f| *f == filter)
                .unwrap_or(0);
            let labels: Vec<&str> = LibraryFilter::ALL.iter().map(|f| f.label()).collect();
            if let Some(index) = widgets::segmented_engraved(&mut actions_ui, &labels, selected) {
                self.library_filter = LibraryFilter::ALL[index];
            }
        }

        // ---- rows ------------------------------------------------------------
        let entries: Vec<LibraryEntry> = self
            .library
            .entries
            .iter()
            .filter(|entry| self.matches_filter(entry, filter))
            .cloned()
            .collect();

        songs::song_columns(ui);

        let current_key = self.current.as_ref().map(LocalLibrary::key_of);

        let mut clicked: Option<(usize, SongAction)> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if entries.is_empty() {
                    let rect = ui.max_rect();
                    widgets::label_at(
                        ui,
                        Pos2::new(rect.center().x, rect.top() + 60.0),
                        Align2::CENTER_CENTER,
                        match filter {
                            LibraryFilter::All => "库里还没有歌 —— 收藏一首，它就会出现在这里",
                            _ => "这个筛选下没有歌",
                        },
                        t::ui_font(12.5),
                        t::FG_3,
                    );
                    return;
                }
                for (index, entry) in entries.iter().enumerate() {
                    let state = self.entry_state(entry);
                    let playing =
                        current_key.as_deref() == Some(LocalLibrary::key_of(&entry.track).as_str());
                    let row = SongRow::from_track(
                        &entry.track,
                        state,
                        added_label(&entry.added_at),
                        playing,
                    );
                    if let Some(action) = songs::song_row(
                        ui,
                        self.theme.accent,
                        &row,
                        index,
                        RowAction::Button(SongAction::Remove),
                    ) {
                        clicked = Some((index, action));
                    }
                }
            });

        if let Some((index, action)) = clicked {
            if let Some(entry) = entries.get(index) {
                let ctx = ui.ctx().clone();
                self.apply_song_action(&ctx, entry, action);
            }
        }
    }

    // -----------------------------------------------------------------------
    // state derived from the library plus what the app has learned
    // -----------------------------------------------------------------------

    /// What the state column should say for a song.
    ///
    /// The design's two interesting states both come from here: 已回退 (the copy
    /// that was meant to play is gone and another platform's is standing in) and
    /// 全部失效 (nothing left). Both are answered by the library's own preference
    /// order, fed with the two things only the app knows: which tracks have
    /// already failed to resolve, and what is already on disk.
    pub(crate) fn entry_state(&self, entry: &LibraryEntry) -> SongState {
        let alive = |track: &crate::api::models::Track| !self.dead.contains(&track.key());
        let cached =
            |track: &crate::api::models::Track| self.cached_state_of(track) == Cached::Complete;
        let Some(picked) = LocalLibrary::preferred(entry, cached, alive) else {
            return SongState::Gone;
        };
        if !alive(&entry.track) {
            // The one that was meant to play is gone; something else is covering it.
            return SongState::Fallback;
        }
        if self.cached_state(entry) == Cached::Partial {
            return SongState::Partial;
        }
        if self.cached_state_of(picked) == Cached::Complete {
            return SongState::Downloaded;
        }
        SongState::Online
    }

    /// Whether what is playing is standing in for a copy that is gone.
    ///
    /// The deck says 已回退 for this, and it is the same question 我的歌's state
    /// column answers — one derivation, two places that show it.
    pub(crate) fn is_fallback(&self, track: &crate::api::models::Track) -> bool {
        self.library
            .find(&LocalLibrary::key_of(track))
            .map(|entry| self.entry_state(entry) == SongState::Fallback)
            .unwrap_or(false)
    }

    /// What is on disk for one song, across all of its versions.
    fn cached_state(&self, entry: &LibraryEntry) -> Cached {
        let mut state = Cached::None;
        for version in &entry.versions {
            match self.cached_state_of(version) {
                Cached::Complete => return Cached::Complete,
                Cached::Partial => state = Cached::Partial,
                Cached::None => {}
            }
        }
        state
    }

    pub(crate) fn cached_state_of(&self, track: &crate::api::models::Track) -> Cached {
        self.cache_index
            .get(&LocalLibrary::key_of(track))
            .map(|info| info.state)
            .unwrap_or(Cached::None)
    }

    fn matches_filter(&self, entry: &LibraryEntry, filter: LibraryFilter) -> bool {
        let state = self.entry_state(entry);
        match filter {
            LibraryFilter::All => true,
            LibraryFilter::Downloaded => self.cached_state(entry) == Cached::Complete,
            LibraryFilter::Stream => {
                self.cached_state(entry) != Cached::Complete && state != SongState::Gone
            }
            LibraryFilter::Issues => matches!(
                state,
                SongState::Gone | SongState::Fallback | SongState::Partial
            ),
        }
    }

    /// Rebuilds the local-copy index for every song in the library.
    ///
    /// Called when the cache or the library changes, never while painting: this is
    /// a few `stat` calls per song, and the list is the screen people look at most.
    pub fn refresh_cache_index(&mut self) {
        self.cache_index.clear();
        for entry in &self.library.entries {
            let key = LocalLibrary::key_of(&entry.track);
            let mut state = Cached::None;
            let mut bytes = 0u64;
            'versions: for version in &entry.versions {
                let cache_key = version.cache_key();
                for tag in QUALITY_TAGS {
                    if let Some(path) = self.worker.cache.get(&cache_key, tag) {
                        // `get` is the same check as `is_complete`, and it hands
                        // back the file, so the size costs nothing extra.
                        state = Cached::Complete;
                        bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
                        break 'versions;
                    }
                    // A download that has begun has a marker and a sidecar but no
                    // fragment yet, so the fragment alone is not the signal.
                    if state == Cached::None
                        && (self.worker.cache.is_downloading(&cache_key, tag)
                            || self.worker.cache.path_for(&cache_key, tag).exists())
                    {
                        state = Cached::Partial;
                    }
                }
            }
            self.cache_index
                .insert(key, crate::app::CacheInfo { state, bytes });
        }
    }

    /// Marks a version as dead, so the library can fall back off it.
    ///
    /// Nothing un-marks it this session: the app has no cheap way to re-probe, and
    /// claiming a copy is alive again without trying it would be a guess. A
    /// restart clears it.
    ///
    /// Public because reaching this state for real takes a failing platform, and
    /// the snapshot example and the tests have to be able to ask for it.
    pub fn mark_dead(&mut self, track: &crate::api::models::Track) {
        self.dead.insert(track.key());
    }

    /// Reacts to a song row, wherever the request came from.
    ///
    /// One dispatcher for the trailing button and the `⋯` menu: they are the same
    /// set of things a person can ask of a row, and splitting them would mean two
    /// places to keep in step.
    fn apply_song_action(&mut self, ctx: &egui::Context, entry: &LibraryEntry, action: SongAction) {
        let track = entry.track.clone();
        let title = track.title.clone();
        match action {
            SongAction::Play => self.start_current(track),
            SongAction::AddToLibrary => {
                if self.library.add(track) {
                    self.save_library();
                    self.refresh_cache_index();
                    self.set_status(format!("已加入我的歌：{title}"), false);
                } else {
                    self.set_status(format!("已经在库里了：{title}"), false);
                }
            }
            SongAction::RemoveFromPlaylist => {
                // The playlist page owns this one: it knows which list is open, and
                // the library itself must not be touched.
            }
            SongAction::Remove => {
                let key = LocalLibrary::key_of(&entry.track);
                if self.library.remove(&key) {
                    self.save_library();
                    self.refresh_cache_index();
                    self.set_status(format!("已从库里移除：{title}"), false);
                }
            }
            SongAction::CopyLink => {
                ctx.copy_text(crate::util::video_url(&track.bvid));
                self.notify("已复制视频链接");
            }
            SongAction::OpenInBrowser => {
                if let Err(err) = crate::platform::open_url(&crate::util::video_url(&track.bvid)) {
                    self.set_status(err, true);
                }
            }
            SongAction::Enqueue | SongAction::EnqueueNext => {
                let next = action == SongAction::EnqueueNext;
                if self.enqueue(track, next) {
                    self.notify(if next {
                        format!("已排在下一首：{title}")
                    } else {
                        format!("已加入播放列表：{title}")
                    });
                } else {
                    self.notify("已经在播放列表里了");
                }
            }
            SongAction::Cache => {
                if self.cache_track(&track) {
                    self.notify(format!("正在缓存：{title}"));
                } else {
                    self.notify("这首正在播放，音频已经在缓存了");
                }
            }
            SongAction::Export => {
                self.send(crate::net::Cmd::ExportTrack(Box::new(track)));
                self.notify(format!("正在导出：{title}"));
            }
            SongAction::DeleteDownload => {
                // Deleting the local copy is the one action here that cannot be
                // undone by syncing, so it is exactly what it says: every quality
                // the app might have written for every version of the song.
                let mut deleted = 0;
                for version in &entry.versions {
                    let cache_key = version.cache_key();
                    for tag in QUALITY_TAGS {
                        if self.worker.cache.is_complete(&cache_key, tag)
                            || self.worker.cache.path_for(&cache_key, tag).exists()
                        {
                            self.worker.cache.remove(&cache_key, tag);
                            self.worker.cache.remove_partial(&cache_key, tag);
                            deleted += 1;
                        }
                    }
                }
                self.refresh_cache_index();
                self.set_status(
                    if deleted == 0 {
                        format!("没有本地副本可删除：{title}")
                    } else {
                        format!("已删除本地副本：{title}")
                    },
                    false,
                );
            }
        }
    }
}

/// The design's 78px page head: title, optional tag, a line of readings, a rule.
///
/// Returns the rect the caller may fill with its own controls, right-aligned.
fn page_head(
    ui: &mut Ui,
    accent: t::Accent,
    title: &str,
    tag: Option<(&str, bool)>,
    meta: &str,
    actions_w: f32,
) -> Rect {
    let (head, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HEAD_H), Sense::hover());
    let painter = ui.painter().clone();
    painter.hline(
        head.x_range(),
        head.bottom() - 0.5,
        Stroke::new(1.0, t::LINE),
    );

    widgets::label_at_bold(
        ui,
        Pos2::new(head.left() + 16.0, head.top() + 26.0),
        Align2::LEFT_CENTER,
        title,
        t::ui_font(19.0),
        t::FG,
    );
    if let Some((text, tone)) = tag {
        let title_w = widgets::measure(ui, title, t::ui_font(19.0)).x;
        let rect = Rect::from_min_size(
            Pos2::new(head.left() + 16.0 + title_w + 12.0, head.top() + 16.0),
            Vec2::new(220.0, 20.0),
        );
        let mut tag_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        widgets::console_tag(&mut tag_ui, text, None, tone, accent);
    }
    widgets::label_at(
        ui,
        Pos2::new(head.left() + 17.0, head.top() + 52.0),
        Align2::LEFT_CENTER,
        meta,
        t::mono_font(10.5),
        t::FG_4,
    );

    Rect::from_min_max(
        Pos2::new(head.right() - 16.0 - actions_w, head.top() + 12.0),
        Pos2::new(head.right() - 16.0, head.top() + 38.0),
    )
}

/// Runs `fill` in a right-to-left layer inside `rect`.
///
/// The page heads are hand-painted, so their controls cannot simply be added to
/// the surrounding layout; this gives them a child ui to live in.
fn in_rect(ui: &mut Ui, rect: Rect, fill: impl FnOnce(&mut Ui)) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    fill(&mut child);
}

/// `2026-08-14 21:03` → `21:03`.
fn clock_of(at: &str) -> String {
    at.split(' ').nth(1).unwrap_or(at).to_owned()
}

impl App {
    /// 最近播放: what you have listened to here, newest first.
    pub(crate) fn ui_recent_page(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let recent = self.library.recent.clone();
        let (today, yesterday) = crate::library::today_and_yesterday();

        let meta = if recent.is_empty() {
            "还没有播放记录".to_owned()
        } else {
            format!("{} 次播放 · 只记这部机器上听过的", recent.len())
        };
        let head = page_head(ui, accent, "最近播放", None, &meta, 104.0);
        let resume = {
            let mut wanted = false;
            in_rect(ui, head, |ui| {
                if widgets::console_button(ui, "继续播放", None, Emphasis::Primary, accent)
                    .clicked()
                {
                    wanted = true;
                }
            });
            wanted
        };

        // The design groups by 今天 / 昨天 / 更早, which is the whole reason the
        // library stores a date with each play.
        let mut groups: [(&str, Vec<usize>); 3] = [
            ("今天", Vec::new()),
            ("昨天", Vec::new()),
            ("更早", Vec::new()),
        ];
        for (index, play) in recent.iter().enumerate() {
            let date = play.at.split(' ').next().unwrap_or_default();
            let bucket = if date == today {
                0
            } else if date == yesterday {
                1
            } else {
                2
            };
            groups[bucket].1.push(index);
        }

        if resume {
            if let Some(play) = recent.first() {
                if let Some(entry) = self.library.find(&play.key).cloned() {
                    self.start_current(entry.track);
                }
            }
        }

        let current_key = self.current.as_ref().map(LocalLibrary::key_of);
        songs::song_columns(ui);
        let mut clicked: Option<(String, SongAction)> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if recent.is_empty() {
                    let rect = ui.max_rect();
                    widgets::label_at(
                        ui,
                        Pos2::new(rect.center().x, rect.top() + 60.0),
                        Align2::CENTER_CENTER,
                        "还没有播放记录 —— 听一首，它就会出现在这里",
                        t::ui_font(12.5),
                        t::FG_3,
                    );
                    return;
                }
                // The number runs across the whole page: restarting it inside each
                // day would show two rows numbered 01 and read as a bug.
                let mut row_number = 0;
                for (label, indexes) in &groups {
                    if indexes.is_empty() {
                        continue;
                    }
                    songs::group_header(ui, label, &format!("{} 首", indexes.len()));
                    for index in indexes.iter() {
                        let play = &recent[*index];
                        let Some(entry) = self.library.find(&play.key) else {
                            continue;
                        };
                        let state = self.entry_state(entry);
                        let playing = current_key.as_deref()
                            == Some(LocalLibrary::key_of(&entry.track).as_str());
                        let row =
                            SongRow::from_track(&entry.track, state, clock_of(&play.at), playing);
                        if let Some(action) =
                            songs::song_row(ui, accent, &row, row_number, RowAction::Menu)
                        {
                            clicked = Some((play.key.clone(), action));
                        }
                        row_number += 1;
                    }
                }
                ui.add_space(18.0);
            });

        if let Some((key, action)) = clicked {
            if let Some(entry) = self.library.find(&key).cloned() {
                let ctx = ui.ctx().clone();
                self.apply_song_action(&ctx, &entry, action);
            }
        }
    }

    /// 歌单: the index, or one playlist.
    pub(crate) fn ui_playlists_page(&mut self, ui: &mut Ui) {
        match self.open_playlist.clone() {
            Some(id) => self.ui_playlist_detail(ui, &id),
            None => self.ui_playlist_index(ui),
        }
    }

    fn ui_playlist_index(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let playlists = self.library.playlists.clone();
        let writable = playlists.iter().filter(|p| !p.readonly).count();
        let meta = format!(
            "{} 个歌单 · {} 个本机可编辑 · {} 个来自平台（只读）",
            playlists.len(),
            writable,
            playlists.len() - writable
        );
        page_head(ui, accent, "歌单", None, &meta, 104.0);

        let mut open: Option<String> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if playlists.is_empty() {
                    let rect = ui.max_rect();
                    widgets::label_at(
                        ui,
                        Pos2::new(rect.center().x, rect.top() + 60.0),
                        Align2::CENTER_CENTER,
                        "还没有歌单 —— 平台收藏会同步成只读歌单",
                        t::ui_font(12.5),
                        t::FG_3,
                    );
                    return;
                }
                for playlist in &playlists {
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width() - 32.0, 58.0),
                        Sense::click(),
                    );
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let painter = ui.painter().clone();
                    if response.hovered() {
                        painter.rect_filled(rect, t::R_SM, t::INK_2);
                    }
                    painter.hline(
                        rect.x_range(),
                        rect.bottom() - 0.5,
                        Stroke::new(1.0, t::LINE),
                    );

                    widgets::label_at_bold(
                        ui,
                        Pos2::new(rect.left() + 4.0, rect.center().y - 8.0),
                        Align2::LEFT_CENTER,
                        &playlist.title,
                        t::ui_font(13.5),
                        t::FG,
                    );
                    let detail = match playlist.origin {
                        crate::library::PlaylistOrigin::Local => {
                            format!("本机 · {} 首 · 可编辑", playlist.entries.len())
                        }
                        crate::library::PlaylistOrigin::Platform(source) => format!(
                            "来自 {} · {} 首 · 只读同步",
                            source.label(),
                            playlist.entries.len()
                        ),
                    };
                    widgets::label_at(
                        ui,
                        Pos2::new(rect.left() + 4.0, rect.center().y + 10.0),
                        Align2::LEFT_CENTER,
                        &detail,
                        t::mono_font(10.0),
                        t::FG_4,
                    );
                    if playlist.readonly {
                        let (tag_rect, _) = (
                            Rect::from_min_size(
                                Pos2::new(rect.right() - 92.0, rect.center().y - 9.0),
                                Vec2::new(70.0, 18.0),
                            ),
                            (),
                        );
                        widgets::paint_label(
                            &painter,
                            Pos2::new(tag_rect.right(), tag_rect.center().y),
                            Align2::RIGHT_CENTER,
                            "只读",
                            t::mono_font(10.0),
                            t::FG_4,
                        );
                    }
                    crate::ui::icons::chevron_right(
                        &painter,
                        Rect::from_center_size(
                            Pos2::new(rect.right() - 18.0, rect.center().y),
                            Vec2::splat(13.0),
                        ),
                        t::FG_4,
                    );
                    if response.clicked() {
                        open = Some(playlist.id.clone());
                    }
                }
                ui.add_space(18.0);
            });

        if let Some(id) = open {
            self.open_playlist = Some(id);
        }
    }

    fn ui_playlist_detail(&mut self, ui: &mut Ui, id: &str) {
        let accent = self.theme.accent;
        let Some(playlist) = self.library.playlists.iter().find(|p| p.id == id).cloned() else {
            self.open_playlist = None;
            return;
        };
        let meta = match playlist.origin {
            crate::library::PlaylistOrigin::Local => {
                format!(
                    "本机歌单 · {} 首 · 在这里增删不会影响任何平台",
                    playlist.entries.len()
                )
            }
            crate::library::PlaylistOrigin::Platform(source) => format!(
                "来自 {} · {} 首 · 下次同步会覆盖本页的改动，所以它是只读的",
                source.label(),
                playlist.entries.len()
            ),
        };
        let head = page_head(
            ui,
            accent,
            &playlist.title,
            Some((
                if playlist.readonly {
                    "只读同步"
                } else {
                    "本机"
                },
                !playlist.readonly,
            )),
            &meta,
            168.0,
        );
        in_rect(ui, head, |ui| {
            if widgets::console_button(ui, "播放", None, Emphasis::Primary, accent).clicked() {
                // handled below via a flag captured by the caller
            }
            ui.add_space(8.0);
            if widgets::console_button(ui, "返回歌单", None, Emphasis::Ghost, accent).clicked()
            {
            }
        });

        // The design's one admission about platform playlists, said quietly: the
        // same note primitive the lyrics column uses for a song with no lyrics.
        if playlist.readonly {
            widgets::note(
                ui,
                crate::ui::icons::lock,
                "这个歌单同步自平台，只能听、不能改 —— 想编辑请复制成自己的歌单。",
            );
        }

        let entries: Vec<crate::library::LibraryEntry> = playlist
            .entries
            .iter()
            .filter_map(|key| self.library.find(key).cloned())
            .collect();
        let current_key = self.current.as_ref().map(LocalLibrary::key_of);
        songs::song_columns(ui);
        let mut clicked: Option<(usize, SongAction)> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if entries.is_empty() {
                    let rect = ui.max_rect();
                    widgets::label_at(
                        ui,
                        Pos2::new(rect.center().x, rect.top() + 60.0),
                        Align2::CENTER_CENTER,
                        "这个歌单还是空的",
                        t::ui_font(12.5),
                        t::FG_3,
                    );
                    return;
                }
                for (index, entry) in entries.iter().enumerate() {
                    let state = self.entry_state(entry);
                    let playing =
                        current_key.as_deref() == Some(LocalLibrary::key_of(&entry.track).as_str());
                    let row = SongRow::from_track(
                        &entry.track,
                        state,
                        added_label(&entry.added_at),
                        playing,
                    );
                    let action = if playlist.readonly {
                        RowAction::Settled
                    } else {
                        RowAction::Button(SongAction::RemoveFromPlaylist)
                    };
                    if let Some(action) = songs::song_row(ui, accent, &row, index, action) {
                        clicked = Some((index, action));
                    }
                }
                ui.add_space(18.0);
            });
        if let Some((index, action)) = clicked {
            if action == SongAction::RemoveFromPlaylist {
                // Out of the list, not out of the library: the two are different
                // collections and the interface must not conflate them.
                if let Some(key) = playlist.entries.get(index) {
                    let key = key.clone();
                    if let Some(list) = self.library.playlists.iter_mut().find(|p| p.id == id) {
                        list.entries.retain(|member| *member != key);
                    }
                    self.save_library();
                    self.set_status("已移出歌单", false);
                }
            } else if let Some(entry) = entries.get(index) {
                let ctx = ui.ctx().clone();
                self.apply_song_action(&ctx, entry, action);
            }
        }
    }

    /// 已下载: the songs with something on disk.
    pub(crate) fn ui_offline_page(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let entries: Vec<crate::library::LibraryEntry> = self
            .library
            .entries
            .iter()
            .filter(|entry| self.cached_state(entry) != Cached::None)
            .cloned()
            .collect();
        let complete = entries
            .iter()
            .filter(|entry| self.cached_state(entry) == Cached::Complete)
            .count();
        let partial = entries.len() - complete;
        let meta = if entries.is_empty() {
            "缓存目录里还没有音频".to_owned()
        } else {
            format!(
                "{} 首有本地音频 · {complete} 首完整 · {partial} 首仅前半",
                entries.len()
            )
        };
        page_head(ui, accent, "已下载", None, &meta, 104.0);

        // Grouped by the first playlist that holds the song, which is what makes a
        // download list navigable rather than a flat pile.
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let key = LocalLibrary::key_of(&entry.track);
            let group = self
                .library
                .playlists
                .iter()
                .find(|playlist| playlist.entries.contains(&key))
                .map(|playlist| playlist.title.clone())
                .unwrap_or_else(|| "不在任何歌单里".to_owned());
            match groups.iter_mut().find(|(title, _)| *title == group) {
                Some((_, members)) => members.push(index),
                None => groups.push((group, vec![index])),
            }
        }

        let current_key = self.current.as_ref().map(LocalLibrary::key_of);
        songs::song_columns(ui);
        let mut clicked: Option<(usize, SongAction)> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if entries.is_empty() {
                    let rect = ui.max_rect();
                    widgets::label_at(
                        ui,
                        Pos2::new(rect.center().x, rect.top() + 60.0),
                        Align2::CENTER_CENTER,
                        "没有下载的音频 —— 边听边缓存的会自动留在这里",
                        t::ui_font(12.5),
                        t::FG_3,
                    );
                    return;
                }
                let mut row_number = 0;
                for (title, members) in &groups {
                    songs::group_header(ui, title, &format!("{} 首", members.len()));
                    for index in members.iter() {
                        let entry = &entries[*index];
                        let state = self.entry_state(entry);
                        let playing = current_key.as_deref()
                            == Some(LocalLibrary::key_of(&entry.track).as_str());
                        let row = SongRow::from_track(
                            &entry.track,
                            state,
                            added_label(&entry.added_at),
                            playing,
                        );
                        if let Some(action) = songs::song_row(
                            ui,
                            accent,
                            &row,
                            row_number,
                            RowAction::Button(SongAction::DeleteDownload),
                        ) {
                            clicked = Some((*index, action));
                        }
                        row_number += 1;
                    }
                }
                ui.add_space(18.0);
            });
        if let Some((index, action)) = clicked {
            if let Some(entry) = entries.get(index) {
                let ctx = ui.ctx().clone();
                self.apply_song_action(&ctx, entry, action);
            }
        }
    }
}

/// 来源与同步: what each platform contributes, and what it is allowed to do.
///
/// The page the library's silence about platforms is paid for: the counts are
/// real, the read/write declarations come from the sources' own capabilities, and
/// the sync switches gate the sync rather than the songs — turning Bilibili off
/// stops reading its favourites, it does not empty the library.
impl App {
    pub(crate) fn ui_sources_page(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let entries = self.library.entries.len();
        let merged = self
            .library
            .entries
            .iter()
            .filter(|entry| entry.versions.len() > 1)
            .count();
        let dead = self
            .library
            .entries
            .iter()
            .filter(|entry| self.entry_state(entry) == SongState::Gone)
            .count()
            + self
                .library
                .entries
                .iter()
                .filter(|entry| self.entry_state(entry) == SongState::Fallback)
                .count();
        let places = self.collection_places();
        let sync = self.library.last_sync.clone();
        let synced_ago = sync
            .as_ref()
            .map(|sync| age_label(sync.at_unix))
            .unwrap_or_else(|| "从未".to_owned());
        let meta = format!("{places} 处收藏 · 合并为 {entries} 首 · 上次同步 {synced_ago}");

        let head = page_head(ui, accent, "来源与同步", None, &meta, 208.0);
        let mut connect = false;
        let mut sync_now = false;
        in_rect(ui, head, |ui| {
            if widgets::console_button(ui, "接入来源", Some(icons::plus), Emphasis::Primary, accent)
                .clicked()
            {
                connect = true;
            }
            ui.add_space(8.0);
            if widgets::console_button(
                ui,
                "立即同步",
                Some(icons::download),
                Emphasis::Ghost,
                accent,
            )
            .clicked()
            {
                sync_now = true;
            }
        });
        if connect {
            self.open_connect_sheet();
        }
        if sync_now {
            if self.user.is_some() {
                self.set_status("正在同步收藏夹与观看历史…", false);
                self.refresh_library();
            } else {
                self.set_status("还没有登录，同步不了任何来源", true);
            }
        }

        let mut toggled: Option<(Source, bool)> = None;

        let mut forget: Option<String> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // ---- what the last sync did ---------------------------------
                let (added, removed) = sync
                    .as_ref()
                    .map(|sync| (sync.added, sync.removed))
                    .unwrap_or((0, 0));
                let row = row_of(ui, 62.0);
                let cells: [(&str, usize, Color32); 5] = [
                    ("新增", added, if added > 0 { t::GREEN } else { t::FG_3 }),
                    ("移除", removed, if removed > 0 { t::FG_2 } else { t::FG_4 }),
                    (
                        "两家都有的合并",
                        merged,
                        if merged > 0 { accent.accent } else { t::FG_4 },
                    ),
                    ("版本失效", dead, if dead > 0 { t::RED } else { t::GREEN }),
                    (
                        "本机可写",
                        self.library
                            .playlists
                            .iter()
                            .filter(|p| !p.readonly)
                            .count(),
                        t::FG_2,
                    ),
                ];
                let cell_w = row.width() / cells.len() as f32;
                for (index, (label, value, color)) in cells.iter().enumerate() {
                    let cell = Rect::from_min_size(
                        Pos2::new(row.left() + cell_w * index as f32, row.top()),
                        Vec2::new(cell_w, row.height()),
                    );
                    widgets::engraved_at(
                        ui,
                        Pos2::new(cell.left() + 12.0, cell.top() + 16.0),
                        Align2::LEFT_CENTER,
                        label,
                        8.5,
                        t::FG_4,
                    );
                    widgets::label_at(
                        ui,
                        Pos2::new(cell.left() + 12.0, cell.top() + 42.0),
                        Align2::LEFT_CENTER,
                        &value.to_string(),
                        t::mono_font(19.0),
                        *color,
                    );
                    if index + 1 < cells.len() {
                        ui.painter().vline(
                            cell.right(),
                            cell.y_range().shrink(14.0),
                            Stroke::new(1.0, t::LINE),
                        );
                    }
                }
                painter_hline(ui, row.bottom(), t::LINE);

                // ---- the notes: only what is true --------------------------
                for note in sync_notes(added, removed, merged, dead, sync.is_some()) {
                    let (line, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
                    let painter = ui.painter().clone();
                    t::led(
                        &painter,
                        Pos2::new(line.left() + 18.0, line.center().y),
                        if note.1 { t::Led::Ok } else { t::Led::Idle },
                        accent,
                    );
                    widgets::label_at(
                        ui,
                        Pos2::new(line.left() + 32.0, line.center().y),
                        Align2::LEFT_CENTER,
                        &note.0,
                        t::ui_font(11.5),
                        t::FG_3,
                    );
                }
                ui.add_space(10.0);

                // ---- one module per source ---------------------------------
                for source in super::nav::PLATFORMS {
                    let synced = self.source_synced(source);
                    let brought = self.collections_of(source);
                    if let Some(want) =
                        source_module(ui, source, synced, brought.0, brought.1, accent)
                    {
                        toggled = Some((source, want));
                    }
                }
                // A declaration gets a module of its own: the design's promise is
                // that the interface grows what a declaration asks for, and the
                // part it cannot grow yet is said rather than implied.
                for declaration in
                    crate::config::with(&self.config, |config| config.declared.clone())
                {
                    if declaration_module(ui, &declaration, accent) {
                        forget = Some(declaration.id.clone());
                    }
                }
                local_module(ui, &self.library, accent);
                ui.add_space(20.0);
            });

        if let Some((source, want)) = toggled {
            self.set_source_synced(source, want);
        }
        if let Some(id) = forget {
            self.remove_declaration(&id);
        }
    }

    /// Writes a declaration into the settings and saves them.
    ///
    /// Public because declaring a source is a real action rather than a test
    /// fixture: 接入来源's form calls this, and so does anything else that means it.
    pub fn declare(&mut self, declaration: crate::config::Declaration) {
        self.update_config(|config| config.declared.push(declaration));
    }

    /// Forgets a declaration.
    ///
    /// Nothing else changes, because nothing else ever did: a declaration this
    /// version understands is a promise about the interface, and the interface is
    /// told to stop making it.
    pub(crate) fn remove_declaration(&mut self, id: &str) {
        let name = crate::config::with(&self.config, |config| {
            config
                .declared
                .iter()
                .find(|declaration| declaration.id == id)
                .map(|declaration| declaration.name.clone())
        });
        self.update_config(|config| {
            config.declared.retain(|declaration| declaration.id != id);
        });
        self.set_status(
            format!(
                "已移除声明：{}（它本来也没有提供任何内容）",
                name.unwrap_or_else(|| id.to_owned())
            ),
            false,
        );
    }

    /// How many places this music is collected from: this machine, plus one per
    /// platform that has actually contributed a collection.
    ///
    /// One derivation, because the rail's badge, 我的歌's tag and 来源与同步's head
    /// are all the same claim — and when they each counted for themselves they
    /// disagreed on an empty library (0 against 1).
    pub(crate) fn collection_places(&self) -> usize {
        if self.library.entries.is_empty() {
            // Nothing collected anywhere yet. Counting the local library as a
            // place would claim a collection that does not exist.
            return 0;
        }
        let mut places = 1;
        for source in Source::ALL {
            let contributed = self.library.playlists.iter().any(|playlist| {
                matches!(
                    playlist.origin,
                    crate::library::PlaylistOrigin::Platform(origin) if origin == source
                )
            });
            if contributed {
                places += 1;
            }
        }
        places
    }

    /// Whether a source's collections are being read.
    pub(crate) fn source_synced(&self, source: Source) -> bool {
        !crate::config::with(&self.config, |config| {
            config.sync_disabled.iter().any(|id| id == source.id())
        })
    }

    /// Stops or resumes reading a source's collections.
    ///
    /// Only the sync: songs already in the library stay playable, because turning
    /// off a source should not delete music — it should stop asking.
    pub(crate) fn set_source_synced(&mut self, source: Source, synced: bool) {
        let id = source.id().to_owned();
        self.update_config(|config| {
            config.sync_disabled.retain(|existing| *existing != id);
            if !synced {
                config.sync_disabled.push(id);
            }
        });
        if synced {
            self.set_status(format!("已恢复同步：{}", source.label()), false);
            self.refresh_library();
        } else {
            self.set_status(
                format!("已停止同步：{}（库里的歌不受影响）", source.label()),
                false,
            );
        }
    }

    /// How many collections this source has contributed, and how many songs.
    fn collections_of(&self, source: Source) -> (usize, usize) {
        let playlists: Vec<&crate::library::Playlist> = self
            .library
            .playlists
            .iter()
            .filter(|playlist| {
                matches!(
                    playlist.origin,
                    crate::library::PlaylistOrigin::Platform(origin) if origin == source
                )
            })
            .collect();
        let songs = playlists
            .iter()
            .map(|playlist| playlist.entries.len())
            .sum();
        (playlists.len(), songs)
    }
}

/// One platform's module: what it brought, what it is allowed to do.
fn source_module(
    ui: &mut Ui,
    source: Source,
    synced: bool,
    collections: usize,
    songs: usize,
    accent: t::Accent,
) -> Option<bool> {
    let capabilities = source.capabilities();
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 104.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(row, 0.0, t::INK_1);
    painter.hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, t::LINE));

    // -- head ---------------------------------------------------------------
    let badge = Rect::from_min_size(
        Pos2::new(row.left() + 18.0, row.top() + 14.0),
        Vec2::splat(26.0),
    );
    painter.rect_filled(badge, t::R_SM, t::INK_3);
    painter.rect_stroke(
        badge,
        t::R_SM,
        Stroke::new(1.0, t::LINE_2),
        egui::StrokeKind::Inside,
    );
    widgets::paint_label(
        &painter,
        badge.center(),
        Align2::CENTER_CENTER,
        source.channel(),
        t::mono_font(12.0),
        t::FG_2,
    );
    widgets::label_at(
        ui,
        Pos2::new(badge.right() + 10.0, badge.center().y - 6.0),
        Align2::LEFT_CENTER,
        source.label(),
        t::ui_font(13.0),
        t::FG,
    );
    t::led(
        &painter,
        Pos2::new(
            badge.right() + 12.0 + widgets::measure(ui, source.label(), t::ui_font(13.0)).x + 8.0,
            badge.center().y - 6.0,
        ),
        if synced { t::Led::Ok } else { t::Led::Idle },
        accent,
    );
    widgets::engraved_at(
        ui,
        Pos2::new(badge.right() + 10.0, badge.center().y + 11.0),
        Align2::LEFT_CENTER,
        source.kind(),
        8.5,
        t::FG_4,
    );

    // -- the read/write declaration, and the switch -------------------------
    let tag = widgets::tag_size(ui, "只读", Some(icons::lock), false);
    let tag_rect = Rect::from_min_size(
        Pos2::new(
            row.right() - 18.0 - 34.0 - 8.0 - tag.x,
            badge.center().y - 10.0,
        ),
        tag,
    );
    widgets::tag_at(
        ui,
        tag_rect,
        if capabilities.write_favorites {
            "可写"
        } else {
            "只读"
        },
        Some(if capabilities.write_favorites {
            icons::folder
        } else {
            icons::lock
        }),
        None,
        if capabilities.write_favorites {
            Some(accent.accent)
        } else {
            None
        },
        accent,
    );

    let mut toggled = None;
    if capabilities.read_favorites {
        let mut on = synced;
        let switch = Rect::from_min_size(
            Pos2::new(row.right() - 18.0 - 34.0, badge.center().y - 10.0),
            Vec2::new(34.0, 20.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(switch)
                .layout(Layout::left_to_right(Align::Center)),
        );
        if widgets::switch(&mut child, &mut on, accent).changed() {
            toggled = Some(on);
        }
    }

    // -- what it brought ----------------------------------------------------
    widgets::engraved_at(
        ui,
        Pos2::new(row.left() + 18.0, row.top() + 56.0),
        Align2::LEFT_CENTER,
        "带来了什么",
        8.5,
        t::FG_4,
    );
    let brought = if !capabilities.read_favorites {
        "没有可读的收藏 —— 这个来源只用来搜索与播放".to_owned()
    } else if collections == 0 {
        if synced {
            "还没有同步过；登录后点「立即同步」".to_owned()
        } else {
            "已停止同步".to_owned()
        }
    } else {
        format!("{collections} 个收藏 · {songs} 首")
    };
    widgets::label_at(
        ui,
        Pos2::new(row.left() + 18.0, row.top() + 76.0),
        Align2::LEFT_CENTER,
        &brought,
        t::ui_font(11.5),
        t::FG_3,
    );

    // -- the capability declaration -----------------------------------------
    let declarations = source.declarations();
    let on = declarations.iter().filter(|(_, on)| *on).count();
    widgets::engraved_at(
        ui,
        Pos2::new(row.right() - 18.0, row.top() + 56.0),
        Align2::RIGHT_CENTER,
        &format!("能力声明 {on} / {}", declarations.len()),
        8.5,
        t::FG_4,
    );
    let detail = if capabilities.write_favorites {
        "可读 ✓ · 可写 ✓ —— 在这里收藏、建歌单都写进本地库".to_owned()
    } else {
        "可读 ✓ · 可写 ✗ —— 别人的收藏，改动要回原处".to_owned()
    };
    widgets::label_at(
        ui,
        Pos2::new(row.right() - 18.0, row.top() + 76.0),
        Align2::RIGHT_CENTER,
        &detail,
        t::mono_font(9.5),
        t::FG_4,
    );

    toggled
}

/// The local library's own module: the one collection this app writes.
/// One declared source: what it asked for, and the one thing it cannot do yet.
///
/// Returns whether 移除 was pressed. There is no switch here, because a switch
/// would imply there is something to switch off.
fn declaration_module(
    ui: &mut Ui,
    declaration: &crate::config::Declaration,
    accent: t::Accent,
) -> bool {
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 104.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(row, 0.0, t::INK_1);
    painter.hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, t::LINE));

    let badge = Rect::from_min_size(
        Pos2::new(row.left() + 18.0, row.top() + 14.0),
        Vec2::splat(26.0),
    );
    painter.rect_filled(badge, t::R_SM, t::INK_3);
    painter.rect_stroke(
        badge,
        t::R_SM,
        Stroke::new(1.0, t::LINE_2),
        egui::StrokeKind::Inside,
    );
    // The channel letter is the badge here, exactly as the form promised.
    widgets::paint_label(
        &painter,
        badge.center(),
        Align2::CENTER_CENTER,
        &declaration.channel,
        t::mono_font(13.0),
        t::FG_2,
    );
    widgets::label_at(
        ui,
        Pos2::new(badge.right() + 10.0, badge.center().y - 6.0),
        Align2::LEFT_CENTER,
        &declaration.name,
        t::ui_font(13.0),
        t::FG,
    );
    t::led(
        &painter,
        Pos2::new(
            badge.right()
                + 10.0
                + widgets::measure(ui, &declaration.name, t::ui_font(13.0)).x
                + 8.0,
            badge.center().y - 6.0,
        ),
        // Amber, not green: it is declared, not working.
        t::Led::Warn,
        accent,
    );
    widgets::engraved_at(
        ui,
        Pos2::new(badge.right() + 10.0, badge.center().y + 11.0),
        Align2::LEFT_CENTER,
        &format!("声明的来源 · 通道 {}", declaration.channel),
        8.5,
        t::FG_4,
    );

    let declared = declaration.declared_count();
    let missing = crate::api::models::CAPABILITIES
        .len()
        .saturating_sub(declared);
    widgets::engraved_at(
        ui,
        Pos2::new(row.right() - 18.0, badge.center().y),
        Align2::RIGHT_CENTER,
        &format!("声明了 {declared} 项能力 · 缺 {missing} 项"),
        9.0,
        t::FG_4,
    );

    widgets::engraved_at(
        ui,
        Pos2::new(row.left() + 18.0, row.top() + 60.0),
        Align2::LEFT_CENTER,
        "带来了什么",
        9.0,
        t::FG_4,
    );
    widgets::label_at(
        ui,
        Pos2::new(row.left() + 18.0, row.top() + 78.0),
        Align2::LEFT_CENTER,
        &format!(
            "还没有实现 —— 本版本不能从这个来源搜索或播放，声明只是让界面知道该长出什么（{}）",
            declaration.session.label()
        ),
        t::ui_font(11.0),
        t::FG_4,
    );
    // Where the other modules put their 可读 / 可写 line. This one has nothing to
    // read or write, so the space belongs to the only action it has.
    let remove = Rect::from_min_size(
        Pos2::new(row.right() - 18.0 - 62.0, row.top() + 78.0 - 12.0),
        Vec2::new(62.0, 24.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(remove));
    widgets::console_button(&mut child, "移除", None, widgets::Emphasis::Ghost, accent).clicked()
}

fn local_module(ui: &mut Ui, library: &crate::library::LocalLibrary, accent: t::Accent) {
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 104.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(row, 0.0, t::INK_1);
    painter.hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, t::LINE));

    let badge = Rect::from_min_size(
        Pos2::new(row.left() + 18.0, row.top() + 14.0),
        Vec2::splat(26.0),
    );
    painter.rect_filled(badge, t::R_SM, t::fade(accent.accent, 0.16));
    painter.rect_stroke(
        badge,
        t::R_SM,
        Stroke::new(1.0, t::fade(accent.accent, 0.4)),
        egui::StrokeKind::Inside,
    );
    icons::folder(
        &painter,
        Rect::from_center_size(badge.center(), Vec2::splat(14.0)),
        accent.accent,
    );
    widgets::label_at(
        ui,
        Pos2::new(badge.right() + 10.0, badge.center().y - 6.0),
        Align2::LEFT_CENTER,
        "本地收藏与歌单",
        t::ui_font(13.0),
        t::FG,
    );
    t::led(
        &painter,
        Pos2::new(
            row.left()
                + 18.0
                + 26.0
                + 10.0
                + widgets::measure(ui, "本地收藏与歌单", t::ui_font(13.0)).x
                + 8.0,
            badge.center().y - 6.0,
        ),
        t::Led::On,
        accent,
    );
    widgets::engraved_at(
        ui,
        Pos2::new(badge.right() + 10.0, badge.center().y + 11.0),
        Align2::LEFT_CENTER,
        "本机文件",
        8.5,
        t::FG_4,
    );

    let writable = library.playlists.iter().filter(|p| !p.readonly).count();
    widgets::engraved_at(
        ui,
        Pos2::new(row.left() + 18.0, row.top() + 56.0),
        Align2::LEFT_CENTER,
        "带来了什么",
        8.5,
        t::FG_4,
    );
    widgets::label_at(
        ui,
        Pos2::new(row.left() + 18.0, row.top() + 76.0),
        Align2::LEFT_CENTER,
        &format!(
            "{} 个歌单 · {} 首 · 收藏也在这里",
            writable,
            library.entries.len()
        ),
        t::ui_font(11.5),
        t::FG_3,
    );
    widgets::engraved_at(
        ui,
        Pos2::new(row.right() - 18.0, row.top() + 56.0),
        Align2::RIGHT_CENTER,
        "本地库 · 全部可写",
        8.5,
        t::FG_4,
    );
    widgets::label_at(
        ui,
        Pos2::new(row.right() - 18.0, row.top() + 76.0),
        Align2::RIGHT_CENTER,
        "可读 ✓ · 可写 ✓ —— 平台同步不会动这里",
        t::mono_font(9.5),
        t::FG_4,
    );
}

/// The notes under the meters, and only the ones that are true.
fn sync_notes(
    added: usize,
    removed: usize,
    merged: usize,
    dead: usize,
    ever_synced: bool,
) -> Vec<(String, bool)> {
    let mut notes = Vec::new();
    if !ever_synced {
        notes.push((
            "还没有同步过 —— 登录来源后点「立即同步」，收藏会按只读同步进来".to_owned(),
            false,
        ));
        return notes;
    }
    if added > 0 || removed > 0 {
        notes.push((
            format!("新增 {added} 首，移除 {removed} 首（平台那边有改动）"),
            true,
        ));
    } else {
        notes.push(("这一次同步没有新增或移除".to_owned(), true));
    }
    if merged > 0 {
        notes.push((format!("{merged} 首在两家都有，库里依然只占一条"), true));
    }
    if dead > 0 {
        notes.push((
            format!("{dead} 首的首选版本已失效，已自动改用另一版接上"),
            true,
        ));
    }
    notes
}

/// How long ago a sync was, in words.
///
/// Not a clock: the library stamps UTC, and a clock read out of it would be
/// wrong by the local offset — which is exactly the sort of quiet wrongness the
/// rest of this app tries not to do. A difference of two UTC instants cannot be.
fn age_label(at_unix: u64) -> String {
    if at_unix == 0 {
        return "从未".to_owned();
    }
    let now = crate::library::now_seconds();
    let seconds = now.saturating_sub(at_unix);
    match seconds {
        0..=59 => "刚刚".to_owned(),
        60..=3599 => format!("{} 分钟前", seconds / 60),
        3600..=86_399 => format!("{} 小时前", seconds / 3600),
        _ => format!("{} 天前", seconds / 86_400),
    }
}

/// The full-width row every block on this page insets into.
fn row_of(ui: &mut Ui, height: f32) -> Rect {
    let (full, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    Rect::from_min_size(
        Pos2::new(full.left() + 16.0, full.top()),
        Vec2::new(full.width() - 32.0, height),
    )
}

fn painter_hline(ui: &Ui, y: f32, color: Color32) {
    let rect = ui.max_rect();
    ui.painter()
        .hline(rect.x_range(), y, Stroke::new(1.0, color));
}

/// 搜索: the same rows as 我的歌, because a song is a song.
///
/// The design's point, and the reason this page is not a separate species: a
/// result already in your library needs nothing done to it, and one that is not
/// needs exactly one thing — 加入我的歌. Nothing here says "试听" or "播放一下就算了".
impl App {
    pub(crate) fn ui_search_page(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent;
        let searching = self.searching;
        let keyword = self.last_keyword.clone();
        let results: Vec<crate::api::models::Track> = self.results.clone();

        // ---- the three states that are not a list of results ---------------
        if searching {
            page_head(
                ui,
                accent,
                &format!("搜索「{keyword}」"),
                Some(("搜索", false)),
                "正在搜索…",
                0.0,
            );
            widgets::skeleton(ui, 6, self.theme.row_h);
            return;
        }
        if results.is_empty() {
            self.search_empty_state(ui, accent, &keyword);
            return;
        }

        // ---- what the search found ----------------------------------------
        let keys: Vec<String> = results.iter().map(LocalLibrary::key_of).collect();
        let mine = keys
            .iter()
            .filter(|key| self.library.find(key).is_some())
            .count();
        let mut sources: Vec<&'static str> = Vec::new();
        for track in &results {
            let label = track.source.label();
            if !sources.contains(&label) {
                sources.push(label);
            }
        }
        let meta = format!(
            "{} 首 · 其中 {mine} 首已在你的库里 · 跨 {} 处来源",
            results.len(),
            sources.len()
        );
        let title = format!("「{keyword}」");

        let head = page_head(ui, accent, &title, Some(("搜索", false)), &meta, 208.0);
        let mut add_all = false;
        let mut play_all = false;
        in_rect(ui, head, |ui| {
            if widgets::console_button(
                ui,
                "全部加入我的歌",
                Some(icons::plus),
                Emphasis::Primary,
                accent,
            )
            .clicked()
            {
                add_all = true;
            }
            ui.add_space(8.0);
            if widgets::console_button(ui, "全部播放", Some(icons::play), Emphasis::Ghost, accent)
                .clicked()
            {
                play_all = true;
            }
        });
        if play_all {
            self.play_from_list(&results, 0);
            return;
        }
        if add_all {
            let mut added = 0;
            for track in &results {
                if self.library.add(track.clone()) {
                    added += 1;
                }
            }
            if added > 0 {
                self.save_library();
                self.refresh_cache_index();
            }
            self.set_status(
                if added == 0 {
                    "这些结果都已在库里".to_owned()
                } else {
                    format!("已加入 {added} 首到我的歌")
                },
                false,
            );
            return;
        }

        // ---- the note, then the rows --------------------------------------
        let mut clicked: Option<(usize, SongAction)> = None;
        let mut load_more = false;
        let scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                widgets::note(
                    ui,
                    icons::search,
                    "搜索结果和「我的歌」长得一样 —— 都是歌。已经在库里的没什么可做的，不在库里的那个动作才是加入我的歌。",
                );
                ui.add_space(6.0);
                songs::song_columns(ui);

                let current_key = self.current.as_ref().map(LocalLibrary::key_of);
                for (index, track) in results.iter().enumerate() {
                    let key = LocalLibrary::key_of(track);
                    let in_library = self.library.find(&key).is_some();
                    let state = self
                        .library
                        .find(&key)
                        .map(|entry| self.entry_state(entry))
                        .unwrap_or(SongState::Online);
                    let playing = current_key.as_deref() == Some(key.as_str());
                    let row = SongRow::from_track(
                        track,
                        state,
                        if in_library { "已在库".to_owned() } else { String::new() },
                        playing,
                    );
                    let action = if in_library {
                        // Nothing to offer a song that is already yours.
                        RowAction::Settled
                    } else {
                        RowAction::Button(SongAction::AddToLibrary)
                    };
                    if let Some(action) = songs::song_row(ui, accent, &row, index, action) {
                        clicked = Some((index, action));
                    }
                }
                if self.loading_more {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.spinner();
                    });
                }
                ui.add_space(20.0);
            });

        if let Some((index, action)) = clicked {
            if let Some(track) = results.get(index).cloned() {
                match action {
                    SongAction::Play => self.play_from_list(&results, index),
                    SongAction::AddToLibrary => {
                        if self.library.add(track) {
                            self.save_library();
                            self.refresh_cache_index();
                        }
                        let title = results[index].title.clone();
                        self.set_status(format!("已加入我的歌：{title}"), false);
                    }
                    other => {
                        // Everything else the row can ask for already has a
                        // dispatcher; a search result that is not in the library
                        // still needs a stand-in entry to be acted on.
                        let mut entry = LibraryEntry {
                            track: track.clone(),
                            versions: vec![track],
                            added_at: String::new(),
                            playlists: Vec::new(),
                            pinned: None,
                        };
                        entry.versions = vec![results[index].clone()];
                        let ctx = ui.ctx().clone();
                        self.apply_song_action(&ctx, &entry, other);
                    }
                }
            }
            return;
        }

        // Scrolling to the end fetches the next page by itself, so the rows are
        // usually already laid out by the time the reader gets there.
        if self.search_has_more
            && !self.loading_more
            && super::near_end(
                scroll.state.offset.y,
                scroll.inner_rect.height(),
                scroll.content_size.y,
            )
        {
            load_more = true;
        }
        if load_more {
            self.loading_more = true;
            self.send(crate::net::Cmd::Search {
                source: self.source,
                keyword: self.last_keyword.clone(),
                page: self.search_page + 1,
            });
        }
    }

    /// What the search pane shows when there is nothing to show.
    fn search_empty_state(&mut self, ui: &mut Ui, accent: t::Accent, keyword: &str) {
        let title = if keyword.is_empty() {
            "搜索".to_owned()
        } else {
            format!("「{keyword}」")
        };
        page_head(ui, accent, &title, Some(("搜索", false)), "", 0.0);
        let examples: &[&str] = &["周杰伦 晴天", "米津玄師", "七里香"];
        // The reader's own recent searches are more useful than built-in examples,
        // but a first run still needs somewhere to start from.
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
    }
}

/// `2026-08-14` → `8 月 14 日`, which is what the design's column holds.
///
/// Anything that does not parse is shown as-is: a wrong date is worse than a
/// plain one, and an old library file may hold either shape.
fn added_label(added_at: &str) -> String {
    let parts: Vec<&str> = added_at.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(month), Ok(day)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
            return format!("{month} 月 {day} 日");
        }
    }
    if added_at.is_empty() {
        String::new()
    } else {
        added_at.to_owned()
    }
}

/// Unused re-exports kept honest by the tests below.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{AudioQuality, Track};
    use crate::app::Page;
    use crate::config::Config;
    use crate::library::{LocalLibrary, Playlist, RecentPlay};
    use egui::{Event, Pos2, RawInput};

    fn raw(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0))),
            events,
            ..Default::default()
        }
    }

    /// The whole shell, exactly as `App::ui` composes it.
    ///
    /// A page cannot be checked on its own: the four parts around it are what the
    /// design is mostly made of, and the deck is where 已回退 and the quality tag
    /// live.
    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<Event>) -> egui::FullOutput {
        let theme = app.theme;
        let mut output = ctx.run_ui(raw(events), |ui| {
            egui::Panel::top("top_bar")
                .exact_size(crate::ui::theme::TOP_BAR_H)
                .frame(crate::ui::frames::top_bar())
                .show(ui, |ui| app.ui_top_bar(ui));
            egui::Panel::bottom("deck")
                .exact_size(crate::ui::theme::DECK_H)
                .frame(crate::ui::frames::deck(&theme))
                .show(ui, |ui| app.ui_player_bar(ui));
            egui::Panel::left("library_nav")
                .exact_size(crate::ui::theme::RAIL_W)
                .frame(crate::ui::frames::library_nav())
                .show(ui, |ui| app.ui_library_nav(ui));
            egui::Panel::right("now_playing")
                .exact_size(crate::ui::theme::NOW_W)
                .frame(crate::ui::frames::now_playing())
                .show(ui, |ui| app.ui_lyrics_panel(ui));
            egui::CentralPanel::default()
                .frame(crate::ui::frames::list_pane())
                .show(ui, |ui| app.ui_central(ui));
        });
        output.textures_delta.clear();
        output
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

    /// Every place a label was painted, so a test can tell the column heading
    /// from a row's button when they share a word.
    fn rects_of(output: &egui::FullOutput, label: &str) -> Vec<Rect> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(text) => {
                    out.push((text.galley.text().to_owned(), text.visual_bounding_rect()));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut all = Vec::new();
        for shape in &output.shapes {
            walk(&shape.shape, &mut all);
        }
        all.into_iter()
            .filter(|(text, _)| text == label)
            .map(|(_, rect)| rect)
            .collect()
    }

    fn track(bvid: &str, source: Source, title: &str) -> Track {
        Track {
            bvid: bvid.to_owned(),
            source,
            aid: 1,
            cid: 2,
            title: title.to_owned(),
            author: "杰威尔音乐".to_owned(),
            duration: 269,
            cover: None,
            album: Some("叶惠美".to_owned()),
            artist: Some("周杰伦".to_owned()),
        }
    }

    /// An app whose library has something for every page to show.
    fn an_app_with_a_library() -> (App, egui::Context, egui::Context) {
        crate::platform::serialise_home_tests();
        std::env::set_var("HOME", "/tmp/listenbli-page-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-page-tests");
        let mut app = App::new_for_tests(Config::default());
        app.library = LocalLibrary::default();

        let bili = track("BV1page", Source::Bilibili, "晴天");
        let youtube = track("yt-page", Source::Youtube, "晴天");
        let seven = track("BV1seven", Source::Bilibili, "七里香");
        app.library.add(bili.clone());
        app.library.add(youtube);
        app.library.add(seven.clone());

        let keys: Vec<String> = app
            .library
            .entries
            .iter()
            .map(|entry| LocalLibrary::key_of(&entry.track))
            .collect();
        let mut local = Playlist::local("late-night", "深夜循环");
        local.entries = keys.clone();
        app.library.playlists.push(local);
        app.library.sync_playlist(
            "platform:bilibili:1",
            "默认收藏夹",
            Source::Bilibili,
            keys.iter().take(2).cloned().collect(),
        );
        app.library.recent = keys
            .iter()
            .map(|key| RecentPlay {
                key: key.clone(),
                at: "2026-08-14 21:03".to_owned(),
            })
            .collect();
        app.show_as_playing(bili, Some(AudioQuality::Flac));

        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);
        (app, ctx, egui::Context::default())
    }

    /// Every page has to draw its own head, on the first pass and after three
    /// frames of settling — and none of them may panic.
    #[test]
    fn every_page_paints_its_own_header() {
        let (mut app, ctx, _) = an_app_with_a_library();
        let pages = [
            (Page::Library, "我的歌", "2 个歌单"),
            (Page::Playlists, "歌单", "个歌单"),
            (Page::Recent, "最近播放", "次播放"),
            (Page::Offline, "已下载", "缓存目录里还没有音频"),
            (Page::Sources, "来源与同步", "处收藏"),
            (Page::Search, "搜索", "输入关键词开始搜索"),
        ];
        for (page, title, headline) in pages {
            app.set_page(page);
            let mut painted = Vec::new();
            for _ in 0..3 {
                painted = texts(&frame(&ctx, &mut app, vec![]));
            }
            assert!(
                painted.iter().any(|text| text == title),
                "{page:?} should paint its title; painted instead: {painted:?}"
            );
            assert!(
                painted.iter().any(|text| text.contains(headline)),
                "{page:?} should paint {headline:?}; painted instead: {painted:?}"
            );
        }
    }

    /// The four parts of the shell are the design, and every page lives inside
    /// them — so they must be there whichever page is up.
    #[test]
    fn the_shell_keeps_its_four_parts_on_every_page() {
        let (mut app, ctx, _) = an_app_with_a_library();
        for page in [
            Page::Library,
            Page::Playlists,
            Page::Recent,
            Page::Offline,
            Page::Search,
            Page::Sources,
        ] {
            app.set_page(page);
            let painted = texts(&frame(&ctx, &mut app, vec![]));
            for part in ["listen", "MY LIBRARY", "NOW PLAYING", "STREAMING"] {
                assert!(
                    painted.iter().any(|text| text == part),
                    "{part} is missing on {page:?}; painted instead: {painted:?}"
                );
            }
        }
    }

    /// 歌单's one admission: a collection synced from a platform cannot be
    /// edited, and a local one can. The difference is a note versus a button.
    #[test]
    fn a_read_only_playlist_offers_no_editing_and_a_local_one_does() {
        let (mut app, ctx, _) = an_app_with_a_library();
        app.set_page(Page::Playlists);

        app.open_playlist = Some("platform:bilibili:1".to_owned());
        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted
                .iter()
                .any(|text| text.contains("这个歌单同步自平台")),
            "a synced collection should say why it is read-only: {painted:?}"
        );
        assert!(
            !painted.iter().any(|text| text == "移出"),
            "and offer nothing that would be undone by the next sync: {painted:?}"
        );

        app.open_playlist = Some("late-night".to_owned());
        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted.iter().any(|text| text == "移出"),
            "a local playlist is editable: {painted:?}"
        );
        assert!(
            !painted
                .iter()
                .any(|text| text.contains("这个歌单同步自平台")),
            "and is not labelled as somebody else's: {painted:?}"
        );
    }

    /// 已下载 is grouped by the playlist a song belongs to, and its rows offer
    /// the one destructive action the app has.
    #[test]
    fn the_offline_page_groups_downloads_and_offers_to_delete_them() {
        let (mut app, ctx, _) = an_app_with_a_library();
        let entry = app
            .library
            .entries
            .iter()
            .find(|entry| entry.track.source == Source::Bilibili)
            .cloned()
            .expect("a Bilibili copy");
        let cache_key = entry.track.cache_key();
        let _ = std::fs::create_dir_all(app.worker.cache.dir());
        let _ = std::fs::write(app.worker.cache.ready_path_for(&cache_key, 30280), b"seed");
        app.refresh_cache_index();

        app.set_page(Page::Offline);
        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted.iter().any(|text| text == "深夜循环"),
            "the group is the playlist the song is in: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "删除"),
            "a downloaded song can be deleted: {painted:?}"
        );
        assert!(
            painted.contains(&entry.track.title),
            "and it is listed: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text.contains("首有本地音频")),
            "the head counts what is on disk: {painted:?}"
        );
    }

    /// 搜索 is the page where the difference between "yours" and "not yours"
    /// shows: one offers 加入, the other has nothing to do.
    #[test]
    fn search_offers_to_add_only_what_is_not_already_yours() {
        let (mut app, ctx, _) = an_app_with_a_library();
        let mine = app.library.entries[0].track.clone();
        let mut stranger = mine.clone();
        stranger.bvid = "BV1stranger".to_owned();
        stranger.title = "龙卷风".to_owned();
        app.show_search_results("周杰伦", vec![mine.clone(), stranger.clone()]);

        let output = frame(&ctx, &mut app, vec![]);
        let painted = texts(&output);
        assert!(
            painted.iter().any(|text| text == "已在库"),
            "a result that is already yours says so: {painted:?}"
        );
        assert!(
            painted
                .iter()
                .any(|text| text.contains("其中 1 首已在你的库里")),
            "the head says how many are already yours: {painted:?}"
        );

        // 「加入」 is both the column heading and the button, so the count has to
        // account for the heading: two labels means exactly one button, and a
        // third would mean a result that is already yours got one too.
        let adds = rects_of(&output, "加入");
        assert_eq!(
            adds.len(),
            2,
            "one heading and one button, nothing more: {painted:?}"
        );
        let mut ys: Vec<f32> = adds.iter().map(|rect| rect.center().y).collect();
        ys.sort_by(f32::total_cmp);
        assert!(
            ys[1] - ys[0] > 20.0,
            "the heading and the button are on different rows: {ys:?}"
        );
    }

    /// The bug the local dates exist for, seen from the interface: a play made
    /// today belongs under 今天 even just after midnight, when the UTC date is
    /// still yesterday.
    #[test]
    fn a_play_made_today_is_grouped_under_today() {
        let (mut app, ctx, _) = an_app_with_a_library();
        let (today, _) = crate::library::today_and_yesterday();
        for play in &mut app.library.recent {
            let clock = play.at.rsplit(' ').next().unwrap_or("21:03").to_owned();
            play.at = format!("{today} {clock}");
        }
        app.set_page(Page::Recent);

        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted.iter().any(|text| text == "今天"),
            "a play stamped today is in 今天: {painted:?}"
        );
        assert!(
            !painted.iter().any(|text| text == "更早"),
            "and not pushed into 更早: {painted:?}"
        );
    }

    /// The defect the empty-library review caught: the rail and the page head
    /// each counted "places" for themselves, and on an empty library one said 0
    /// while the other said 1.
    #[test]
    fn the_places_count_is_one_claim_in_both_places() {
        let (mut app, ctx, _) = an_app_with_a_library();
        // This machine plus Bilibili, which is the only platform with a
        // collection here.
        assert_eq!(app.collection_places(), 2);

        app.set_page(Page::Library);
        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted.iter().any(|text| text == "跨 2 处收藏"),
            "the page tag: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "2 处收藏"),
            "and the rail says the same thing: {painted:?}"
        );

        // Nothing collected anywhere: both have to say zero rather than one.
        app.library = LocalLibrary::default();
        assert_eq!(app.collection_places(), 0);
        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted.iter().any(|text| text == "跨 0 处收藏"),
            "an empty library is collected in no places: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "0 处收藏"),
            "and the rail agrees: {painted:?}"
        );
    }

    /// The design's promise is that a declaration makes the interface grow the
    /// part it asks for. What it cannot grow yet is said on the card, because a
    /// source that silently provides nothing would be worse than no source.
    #[test]
    fn a_declared_source_is_shown_and_says_it_is_not_implemented() {
        let (mut app, ctx, _) = an_app_with_a_library();
        app.declare(crate::config::Declaration {
            id: "archive-org".to_owned(),
            name: "Internet Archive".to_owned(),
            channel: "I".to_owned(),
            capabilities: vec!["search".to_owned(), "stream".to_owned()],
            session: crate::config::Session::Anonymous,
        });
        app.set_page(Page::Sources);

        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            painted.iter().any(|text| text == "Internet Archive"),
            "the declared source has a card: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "I"),
            "badged with the channel letter the form promised: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text.contains("还没有实现")),
            "and says what it cannot do: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text.contains("声明了 2 项能力")),
            "counted from its own declaration: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "移除"),
            "with the one action it has: {painted:?}"
        );

        // It contributes nothing: no songs, no collection, so the count of places
        // does not move. This machine and Bilibili, exactly as before.
        assert_eq!(app.collection_places(), 2);

        app.remove_declaration("archive-org");
        let painted = texts(&frame(&ctx, &mut app, vec![]));
        assert!(
            !painted.iter().any(|text| text == "Internet Archive"),
            "and removing the declaration takes the card away: {painted:?}"
        );
    }

    /// A narrow window is the case the design's own canvas caught: the rail and
    /// the now-playing column are fixed width, so the page in between is what
    /// suffers. Drawing at 900px must not panic, and the page must still say what
    /// it is.
    #[test]
    fn a_narrow_window_still_draws_a_page() {
        let (mut app, ctx, _) = an_app_with_a_library();
        let narrow = egui::Context::default();
        crate::ui::theme::install_fonts(&narrow, None);
        crate::ui::theme::install_style(&narrow, &app.theme);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
            ..Default::default()
        };
        for page in [Page::Library, Page::Sources, Page::Offline] {
            app.set_page(page);
            let mut output = narrow.run_ui(raw.clone(), |ui| {
                egui::Panel::left("library_nav")
                    .exact_size(crate::ui::theme::RAIL_W)
                    .frame(crate::ui::frames::library_nav())
                    .show(ui, |ui| app.ui_library_nav(ui));
                egui::CentralPanel::default()
                    .frame(crate::ui::frames::list_pane())
                    .show(ui, |ui| app.ui_central(ui));
            });
            output.textures_delta.clear();
            assert!(
                !texts(&output).is_empty(),
                "{page:?} painted nothing at 900px"
            );
        }
        let _ = ctx;
    }

    #[test]
    fn a_library_date_is_written_the_way_the_design_writes_it() {
        assert_eq!(added_label("2026-08-14"), "8 月 14 日");
        assert_eq!(added_label("2026-10-01"), "10 月 1 日");
    }

    #[test]
    fn a_date_that_does_not_parse_is_left_alone() {
        // An older library file, or a hand-edited one, must not be mangled.
        assert_eq!(added_label("上周"), "上周");
        assert_eq!(added_label("2026"), "2026");
        assert_eq!(added_label(""), "");
    }

    #[test]
    fn a_sync_age_is_written_in_words_rather_than_a_clock() {
        // The library stamps UTC and std has no timezone, so a difference is the
        // only reading that cannot be hours out.
        let now = crate::library::now_seconds();
        assert_eq!(super::age_label(0), "从未");
        assert_eq!(super::age_label(now), "刚刚");
        assert_eq!(super::age_label(now - 120), "2 分钟前");
        assert_eq!(super::age_label(now - 3 * 3600), "3 小时前");
        assert_eq!(super::age_label(now - 2 * 86_400), "2 天前");
        // A stamp from the future (a clock that stepped back) reads as 刚刚
        // rather than as a negative age.
        assert_eq!(super::age_label(now + 600), "刚刚");
    }

    #[test]
    fn the_meta_line_reports_what_the_page_is_showing() {
        // Just the shape: the counts come from the library, and the string is what
        // a person reads to know whether the list is complete.
        let meta = format!("{} 首 · {} 个歌单 · 已下载 {} 首", 214, 18, 17);
        assert!(meta.starts_with("214 首"));
        assert!(meta.contains("已下载 17 首"));
    }
}

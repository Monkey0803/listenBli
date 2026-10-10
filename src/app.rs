//! Application state, event pump and top-level layout.
//!
//! The rendering code for each panel lives in `crate::ui::*` as further
//! `impl App` blocks, so this file stays about state and plumbing.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{ColorImage, TextureHandle, TextureOptions};

use crate::api::client::Api;
use crate::api::models::{AudioQuality, FavFolder, Track, UserInfo};
use crate::audio::{AudioEngine, SeekOutcome};
use crate::config::{Config, SharedConfig};
use crate::lyrics::Lyrics;
use crate::net::{self, Cmd, Evt, Worker};
use crate::platform;
use crate::ui::theme::Theme;

/// How long a status message stays on screen.
pub(crate) const STATUS_TTL: Duration = Duration::from_secs(6);
/// How long the "copied" toast stays up.
pub(crate) const TOAST_TTL: Duration = Duration::from_millis(1600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Search,
    Favorites,
    History,
}

pub struct Status {
    pub text: String,
    pub is_error: bool,
    pub at: Instant,
}

/// State for the QR login modal.
pub struct QrDialog {
    pub key: String,
    pub image: Option<ColorImage>,
    pub texture: Option<TextureHandle>,
    pub message: String,
    pub last_poll: Instant,
    pub finished: bool,
}

pub struct App {
    pub(crate) worker: Worker,
    pub(crate) config: SharedConfig,

    pub(crate) engine: Option<AudioEngine>,
    pub(crate) engine_error: Option<String>,

    pub(crate) tab: Tab,
    /// Which platform the list is showing. Persisted, because a user who came for
    /// YouTube should not have to say so again on every launch.
    pub(crate) source: crate::api::models::Source,

    pub(crate) search_input: String,
    pub(crate) searching: bool,
    /// A page *after* the first one is in flight, so the list stays put. Set
    /// the moment the request leaves the UI and cleared by the reply, so the
    /// automatic "scrolled to the end" fetch cannot ask for one page twice.
    pub(crate) loading_more: bool,
    pub(crate) last_keyword: String,
    pub(crate) results: Vec<Track>,
    /// Highest result page loaded for `last_keyword`.
    pub(crate) search_page: u32,
    pub(crate) search_has_more: bool,

    pub(crate) queue: Vec<Track>,
    pub(crate) queue_pos: usize,
    pub(crate) current: Option<Track>,
    pub(crate) quality: Option<AudioQuality>,
    /// True from the moment a track is picked until its audio is playable.
    pub(crate) loading: bool,
    /// `(track key, bytes downloaded, total bytes)` while a download runs.
    pub(crate) download: Option<(String, u64, Option<u64>)>,

    pub(crate) covers: HashMap<String, TextureHandle>,
    pub(crate) requested_covers: HashSet<String>,

    pub(crate) lyrics: Lyrics,
    pub(crate) lyrics_for: Option<String>,
    pub(crate) follow_lyrics: bool,
    pub(crate) manual_scroll_at: Option<Instant>,
    pub(crate) show_translation: bool,

    pub(crate) user: Option<UserInfo>,
    pub(crate) qr: Option<QrDialog>,

    pub(crate) fav_folders: Vec<FavFolder>,
    pub(crate) fav_items: Vec<Track>,
    pub(crate) selected_folder: Option<i64>,
    pub(crate) fav_has_more: bool,
    pub(crate) fav_page: u32,
    /// Distinguishes "still loading" from "genuinely empty".
    pub(crate) fav_loaded: bool,
    pub(crate) history: Vec<Track>,
    pub(crate) history_loaded: bool,

    pub(crate) status: Option<Status>,
    pub(crate) volume: f32,

    /// Design tokens derived from the config (accent, lyric size, density).
    pub(crate) theme: Theme,
    /// Which overlays are open. Only one sheet can be open at a time.
    pub(crate) settings_open: bool,
    pub(crate) account_menu_open: bool,
    pub(crate) queue_open: bool,
    /// A transient confirmation message, e.g. "已复制到剪贴板".
    pub(crate) toast: Option<(String, Instant)>,
    /// Set by ⌘K; consumed by the search field on the next frame.
    pub(crate) focus_search: bool,
    /// Whether the recent-search dropdown under the search field is showing.
    /// Kept here rather than derived from focus, so clicking an entry does not
    /// dismiss the list before the click is resolved.
    pub(crate) search_history_open: bool,
    /// Identifies the installed font set so a settings change can be detected.
    pub(crate) fonts_key: String,
    /// Identifies the applied widget style, so it is only rebuilt on a change.
    pub(crate) style_key: String,
    /// In-progress text of the settings sheet's font-path field.
    pub(crate) cjk_path_input: String,
    /// Bytes under the cache directory, measured when the settings sheet opens
    /// (a directory walk is cheap but should not run every frame).
    pub(crate) cache_bytes: Option<u64>,
    /// In-progress text of the settings sheet's cache-directory field. Empty
    /// means "use the platform default".
    pub(crate) cache_path_input: String,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self::build(config, true)
    }

    /// Build an app that never opens an audio output.
    ///
    /// The unit tests run in parallel, and several `AudioEngine`s asking Windows
    /// for an output device at the same time takes the whole test binary down
    /// (STATUS_ACCESS_VIOLATION) — the app itself only ever opens one engine, at
    /// startup. Nothing the tests assert needs the device, so they take this
    /// path and leave `engine_error` unset.
    #[cfg(test)]
    pub(crate) fn new_for_tests(config: Config) -> Self {
        Self::build(config, false)
    }

    fn build(config: Config, with_audio: bool) -> Self {
        let volume = config.volume;
        let show_translation = config.prefer_translation;
        let theme = Theme::from_config(&config);
        let fonts_key = crate::ui::theme::fonts_key(&config);
        let source = config.source;
        let cache_root = platform::resolve_cache_dir(config.cache_dir.as_deref());
        let cache_path_input = config
            .cache_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let config_path_input = config
            .cjk_font_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let shared: SharedConfig = Arc::new(std::sync::Mutex::new(config));

        // Reuse the persisted cookie jar so a previous login survives restarts.
        let jar = shared.lock().unwrap().cookies.clone();
        let api = Arc::new(Api::new(jar));

        let (engine, engine_error) = if with_audio {
            match AudioEngine::new(volume) {
                Ok(engine) => (Some(engine), None),
                Err(err) => (None, Some(err)),
            }
        } else {
            (None, None)
        };

        let worker = net::spawn(Arc::clone(&api), Arc::clone(&shared));
        // The download worker starts on the platform default; honour a configured
        // root before anything can be requested.
        worker.cache.set_root(&cache_root);
        // Ask the worker who we are; it also refreshes the WBI keys.
        let _ = worker.cmd_tx.send(Cmd::RefreshLogin);

        Self {
            worker,
            config: shared,
            engine,
            engine_error,
            tab: Tab::Search,
            source,
            search_input: String::new(),
            searching: false,
            loading_more: false,
            last_keyword: String::new(),
            results: Vec::new(),
            search_page: 1,
            search_has_more: false,
            queue: Vec::new(),
            queue_pos: 0,
            current: None,
            quality: None,
            loading: false,
            download: None,
            covers: HashMap::new(),
            requested_covers: HashSet::new(),
            lyrics: Lyrics::empty(crate::lyrics::LyricsSource::None),
            lyrics_for: None,
            follow_lyrics: true,
            manual_scroll_at: None,
            show_translation,
            user: None,
            qr: None,
            fav_folders: Vec::new(),
            fav_items: Vec::new(),
            selected_folder: None,
            fav_has_more: false,
            fav_page: 1,
            fav_loaded: false,
            history: Vec::new(),
            history_loaded: false,
            status: None,
            volume,
            theme,
            settings_open: false,
            account_menu_open: false,
            queue_open: false,
            toast: None,
            focus_search: false,
            search_history_open: false,
            fonts_key,
            style_key: String::new(),
            cjk_path_input: config_path_input,
            cache_bytes: None,
            cache_path_input,
        }
    }

    // -- helpers -----------------------------------------------------------

    pub(crate) fn send(&self, cmd: Cmd) {
        // A dead worker means the app is shutting down; nothing useful to do.
        let _ = self.worker.cmd_tx.send(cmd);
    }

    /// Mutate the persisted configuration and write it back. Every settings
    /// control funnels through here so nothing is saved twice.
    pub(crate) fn update_config(&mut self, change: impl FnOnce(&mut Config)) {
        let Ok(mut guard) = self.config.lock() else {
            return;
        };
        change(&mut guard);
        if let Err(err) = guard.save() {
            eprintln!("saving config failed: {err}");
        }
    }

    /// Mutate the in-memory configuration without touching the disk. Used for
    /// live previews, which are persisted once the interaction ends.
    pub(crate) fn set_config_in_memory(&mut self, change: impl FnOnce(&mut Config)) {
        if let Ok(mut guard) = self.config.lock() {
            change(&mut guard);
        }
    }

    pub(crate) fn config_value<R>(&self, read: impl FnOnce(&Config) -> R) -> R {
        let guard = self.config.lock().unwrap();
        read(&guard)
    }

    /// Where cached audio and lyrics go right now.
    pub(crate) fn cache_root(&self) -> std::path::PathBuf {
        self.config_value(|config| platform::resolve_cache_dir(config.cache_dir.as_deref()))
    }

    /// Point the cache at `path`, or back at the platform default when `None`.
    ///
    /// The directory is created and probed for writability *before* it is
    /// accepted: a location the app cannot write to would otherwise surface as
    /// one failed download at a time, which is a poor way to find out.
    ///
    /// Nothing is moved. Existing files stay where they are, so a user who
    /// switches back finds them, and one who does not can delete them.
    pub(crate) fn set_cache_dir(&mut self, path: Option<std::path::PathBuf>) {
        let chosen = path.filter(|path| !path.as_os_str().is_empty());
        let root = platform::resolve_cache_dir(chosen.as_deref());

        if !platform::is_writable_dir(&root) {
            self.cache_path_input = chosen
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            self.notify(format!("无法写入 {}，已保持原目录", root.display()));
            return;
        }

        self.worker.cache.set_root(&root);
        self.update_config(|config| config.cache_dir = chosen.clone());
        // Re-measure on the next frame, now that the sheet is still open.
        self.cache_bytes = None;
        self.notify(if chosen.is_some() {
            "缓存目录已更改，已有文件不会搬动"
        } else {
            "缓存目录已恢复默认位置"
        });
    }

    // -- search history ----------------------------------------------------

    /// The recent search keywords, newest first.
    ///
    /// The configuration is the only place they live, so the list the panel
    /// shows can never drift from the one on disk.
    pub(crate) fn search_history(&self) -> Vec<String> {
        self.config_value(|config| config.search_history.clone())
    }

    /// Record a search the user actually ran.
    pub(crate) fn remember_search(&mut self, keyword: &str) {
        self.update_config(|config| config.remember_search(keyword));
    }

    pub(crate) fn clear_search_history(&mut self) {
        self.update_config(|config| config.search_history.clear());
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.current.is_some()
            && !self.loading
            && self
                .engine
                .as_ref()
                .is_some_and(|engine| !engine.is_paused() && !engine.is_finished())
    }

    pub(crate) fn position(&self) -> Duration {
        self.engine
            .as_ref()
            .map(|engine| engine.position())
            .unwrap_or_default()
    }

    pub(crate) fn duration(&self) -> Duration {
        self.engine
            .as_ref()
            .map(|engine| engine.duration())
            .filter(|d| !d.is_zero())
            .or_else(|| {
                self.current
                    .as_ref()
                    .map(|t| Duration::from_secs(t.duration))
            })
            .unwrap_or_default()
    }

    pub(crate) fn progress(&self) -> f32 {
        let duration = self.duration().as_secs_f32();
        if duration <= 0.0 {
            0.0
        } else {
            (self.position().as_secs_f32() / duration).clamp(0.0, 1.0)
        }
    }

    /// How much of the file is on disk, for the seek bar's buffered segment.
    pub(crate) fn buffered(&self) -> Option<f32> {
        let (_, got, total) = self.download.as_ref()?;
        total
            .filter(|total| *total > 0)
            .map(|total| (*got as f32 / total as f32).clamp(0.0, 1.0))
    }

    pub(crate) fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    /// Open one of the app's overlays by name: `settings`, `queue` or `login`.
    ///
    /// Exposed for tooling (see `examples/ui_snapshot.rs`) rather than used by
    /// the shipping UI, which opens them from their own controls.
    pub fn open_overlay(&mut self, name: &str) {
        match name {
            "settings" => self.settings_open = true,
            "queue" => self.queue_open = true,
            "login" => self.open_login(),
            _ => {}
        }
    }

    pub(crate) fn toggle_pause(&mut self) {
        if let Some(engine) = &self.engine {
            engine.toggle_pause();
        }
    }

    pub(crate) fn set_status(&mut self, text: impl Into<String>, is_error: bool) {
        self.status = Some(Status {
            text: text.into(),
            is_error,
            at: Instant::now(),
        });
    }

    /// Make `track` the current one, queue it and start resolving it.
    pub(crate) fn play_track(&mut self, track: Track, queue: Vec<Track>, position: usize) {
        self.queue = queue;
        self.queue_pos = position;
        self.start_current(track);
    }

    fn start_current(&mut self, track: Track) {
        self.lyrics = Lyrics::empty(crate::lyrics::LyricsSource::None);
        self.lyrics_for = None;
        self.follow_lyrics = true;
        self.manual_scroll_at = None;
        self.quality = None;
        self.loading = true;
        self.request_cover(&track);
        self.current = Some(track.clone());
        self.send(Cmd::LoadTrack(Box::new(track)));
    }

    pub(crate) fn play_next(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let next = (self.queue_pos + 1) % self.queue.len();
        // Wrapping is only desirable for an explicit "next" click, not for
        // auto-advance at the end of the queue.
        if next == 0 && self.queue_pos + 1 >= self.queue.len() && self.queue.len() > 1 {
            return;
        }
        self.queue_pos = next;
        let track = self.queue[next].clone();
        self.start_current(track);
    }

    pub(crate) fn play_prev(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.queue_pos = if self.queue_pos == 0 {
            self.queue.len() - 1
        } else {
            self.queue_pos - 1
        };
        let track = self.queue[self.queue_pos].clone();
        self.start_current(track);
    }

    pub(crate) fn request_cover(&mut self, track: &Track) {
        let key = track.key();
        if self.covers.contains_key(&key) || self.requested_covers.contains(&key) {
            return;
        }
        if let Some(url) = track.cover.clone().filter(|u| !u.is_empty()) {
            self.requested_covers.insert(key.clone());
            self.worker.fetch_cover(key, url);
        }
    }

    pub(crate) fn is_current(&self, track: &Track) -> bool {
        self.current
            .as_ref()
            .is_some_and(|c| c.key() == track.key())
    }

    /// Queue a whole list (search results / favourites / history) starting at
    /// `index`.
    pub(crate) fn play_from_list(&mut self, list: &[Track], index: usize) {
        if let Some(track) = list.get(index).cloned() {
            self.play_track(track, list.to_vec(), index);
        }
    }

    /// Put `track` in the play queue: at the end, or right after the current one.
    ///
    /// Returns `false` when the queue already holds it — asking twice for the
    /// same song should not queue it twice.
    pub(crate) fn enqueue(&mut self, track: Track, next: bool) -> bool {
        if self.queue.iter().any(|queued| queued.key() == track.key()) {
            return false;
        }
        if next && !self.queue.is_empty() {
            let at = (self.queue_pos + 1).min(self.queue.len());
            self.queue.insert(at, track);
        } else {
            self.queue.push(track);
        }
        true
    }

    /// Ask the worker to cache `track` without touching playback.
    ///
    /// Refuses while the same track is being streamed into the cache: two
    /// writers on one cache file would corrupt it, and the stream already ends
    /// up cached anyway.
    pub(crate) fn cache_track(&mut self, track: &Track) -> bool {
        let streaming = self
            .download
            .as_ref()
            .is_some_and(|(key, ..)| *key == track.key());
        if streaming {
            return false;
        }
        self.send(Cmd::CacheTrack(Box::new(track.clone())));
        true
    }

    /// Seek without ever freezing the UI thread.
    ///
    /// The decision lives in the engine, which knows whether the installed
    /// decoder can seek at all. A track that is still arriving plays through an
    /// unseekable decoder (that is what lets it start early), so a jump backwards
    /// waits for the file to complete; the engine reports which of those
    /// happened and this only turns it into a message.
    ///
    /// Nothing is loaded while a decoder is still being built, so a seek then
    /// has no source to move and is refused rather than handed to the player.
    pub(crate) fn try_seek(&mut self, target: Duration) {
        if self.loading {
            return;
        }
        let Some(engine) = self.engine.as_mut() else {
            // No output device: nothing can move, and saying so beats silence.
            self.set_status("音频输出不可用，无法跳转", true);
            return;
        };
        match engine.seek(target) {
            SeekOutcome::Seeked => {}
            SeekOutcome::Upgrading => {
                self.set_status("正在打开可跳转的副本，稍后跳转…", false);
            }
            SeekOutcome::Refused => {
                self.set_status("音频仍在缓冲，暂时无法回退到该位置", false);
            }
        }
    }

    pub(crate) fn try_seek_fraction(&mut self, fraction: f32) {
        let Some(duration) = self.engine.as_ref().map(|engine| engine.duration()) else {
            return;
        };
        let target = duration.mul_f32(fraction.clamp(0.0, 1.0));
        self.try_seek(target);
    }

    // -- event pump --------------------------------------------------------

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(evt) = self.worker.evt_rx.try_recv() {
            self.handle_event(ctx, evt);
        }
    }

    fn handle_event(&mut self, ctx: &egui::Context, evt: Evt) {
        match evt {
            Evt::SearchStarted { keyword, page } => {
                if page <= 1 {
                    self.searching = true;
                    self.loading_more = false;
                    self.last_keyword = keyword;
                    self.results.clear();
                    self.search_page = 1;
                    self.search_has_more = false;
                } else {
                    // Appending: the rows already on screen must stay there.
                    self.loading_more = true;
                }
            }
            Evt::SearchResults {
                keyword,
                page,
                tracks,
                has_more,
            } => {
                self.searching = false;
                self.loading_more = false;
                // Ignore a late response for a query the user already replaced.
                if keyword != self.last_keyword {
                    return;
                }
                let mut has_more = has_more;
                if page <= 1 {
                    self.results = tracks;
                } else {
                    // Bilibili's ranking is not stable between requests: live
                    // runs showed page 2 repeating several of page 1's videos,
                    // so appending has to skip what is already listed.
                    let before = self.results.len();
                    for track in tracks {
                        let already_listed = self
                            .results
                            .iter()
                            .any(|listed| listed.key() == track.key());
                        if !already_listed {
                            self.results.push(track);
                        }
                    }
                    // The server clamps an out-of-range page rather than
                    // returning an empty one, so "this page added nothing" is
                    // the reliable end-of-list signal.
                    if self.results.len() == before {
                        has_more = false;
                    }
                }
                self.search_page = page.max(1);
                self.search_has_more = has_more;
                let snapshot = self.results.clone();
                for track in &snapshot {
                    self.request_cover(track);
                }
                if self.results.is_empty() {
                    self.set_status("没有找到结果", false);
                }
            }
            Evt::TrackResolved(track) => {
                // Show the title immediately; the audio may still be downloading.
                self.current = Some(*track);
            }
            Evt::TrackReady {
                track,
                source,
                quality,
            } => {
                // Deliberately still loading: the decoder is built on a worker
                // thread and only installed from `tick`, and nothing can play
                // until that has happened.
                self.quality = Some(quality);
                let key = track.key();
                self.current = Some(*track.clone());
                match &mut self.engine {
                    Some(engine) => {
                        engine.play_stream(&source, Duration::from_secs(track.duration), key);
                    }
                    None => {
                        self.loading = false;
                        self.set_status("音频输出不可用，无法播放", true);
                    }
                }
            }
            Evt::DownloadProgress { key, got, total } => {
                // A finished download is not "progress" any more; dropping it
                // clears the cache row and the seek bar's buffered segment.
                self.download = match total {
                    Some(total) if total > 0 && got >= total => None,
                    _ => Some((key, got, total)),
                };
            }
            Evt::LyricsReady { key, lyrics } => {
                // Only apply lyrics that belong to the track still selected.
                if self.current.as_ref().is_some_and(|t| t.key() == key) {
                    self.lyrics = lyrics;
                    self.lyrics_for = Some(key);
                    self.follow_lyrics = true;
                }
            }
            Evt::CoverReady { key, image } => {
                let texture =
                    ctx.load_texture(format!("cover-{key}"), image, TextureOptions::LINEAR);
                self.covers.insert(key, texture);
            }
            Evt::QrCode { key, image } => {
                if let Some(dialog) = &mut self.qr {
                    dialog.key = key;
                    dialog.texture =
                        Some(ctx.load_texture("login-qr", image.clone(), TextureOptions::NEAREST));
                    dialog.image = Some(image);
                    dialog.last_poll = Instant::now();
                    dialog.finished = false;
                } else {
                    let texture =
                        ctx.load_texture("login-qr", image.clone(), TextureOptions::NEAREST);
                    self.qr = Some(QrDialog {
                        key,
                        image: Some(image),
                        texture: Some(texture),
                        message: "请使用哔哩哔哩手机客户端扫码".into(),
                        last_poll: Instant::now(),
                        finished: false,
                    });
                }
            }
            Evt::LoginStatus { message, done } => {
                if let Some(dialog) = &mut self.qr {
                    dialog.message = message.clone();
                    if done {
                        dialog.finished = true;
                    }
                }
                if done {
                    if !message.contains("过期") {
                        self.qr = None;
                    }
                    self.set_status(message, false);
                }
            }
            Evt::LoginChanged { user } => {
                let logged_in = user.is_some();
                self.user = user;
                if logged_in {
                    self.set_status("已登录", false);
                    self.refresh_library();
                } else {
                    self.fav_folders.clear();
                    self.fav_items.clear();
                    self.selected_folder = None;
                    self.fav_loaded = false;
                    self.history.clear();
                    self.history_loaded = false;
                }
            }
            Evt::FavFolders(folders) => {
                self.selected_folder = folders.first().map(|f| f.id);
                self.fav_folders = folders;
                self.fav_page = 1;
                if let Some(id) = self.selected_folder {
                    self.send(Cmd::LoadFavItems {
                        media_id: id,
                        page: 1,
                    });
                }
            }
            Evt::FavItems {
                media_id,
                page,
                tracks,
                has_more,
            } => {
                if Some(media_id) != self.selected_folder {
                    return;
                }
                self.loading_more = false;
                if page <= 1 {
                    self.fav_items = tracks;
                } else {
                    self.fav_items.extend(tracks);
                }
                self.fav_page = page.max(1);
                self.fav_has_more = has_more;
                self.fav_loaded = true;
                let snapshot = self.fav_items.clone();
                for track in &snapshot {
                    self.request_cover(track);
                }
            }
            Evt::History(tracks) => {
                self.history = tracks;
                self.history_loaded = true;
                let snapshot = self.history.clone();
                for track in &snapshot {
                    self.request_cover(track);
                }
            }
            Evt::Exported { title, path } => {
                // The point of the export is the file, so name it: the toast is
                // the only place the user can read where it landed.
                self.notify(format!("已导出 {title}：{}", path.display()));
            }
            Evt::Cached { title, already } => {
                // A cache-only download finished in the background: nothing on
                // screen depended on it, so a toast is the whole reaction.
                self.notify(if already {
                    format!("已在缓存中：{title}")
                } else {
                    format!("已缓存：{title}")
                });
            }
            Evt::Error { context, message } => {
                self.loading = false;
                self.searching = false;
                self.loading_more = false;
                self.download = None;
                self.set_status(format!("{context}：{message}"), true);
            }
            Evt::Info(message) => {
                self.set_status(message, false);
            }
        }
    }

    fn refresh_library(&mut self) {
        if let Some(user) = &self.user {
            self.send(Cmd::LoadFavFolders { mid: user.mid });
            self.send(Cmd::LoadHistory);
        }
    }

    /// Drive auto-advance and QR polling.
    fn tick(&mut self) {
        // Install a decoder whose worker thread has finished with it. This is
        // where playback actually starts; everything before it is preparation,
        // and the UI stays live throughout.
        match self.engine.as_mut().and_then(|engine| engine.poll()) {
            Some(Ok(())) => self.loading = false,
            Some(Err(err)) => {
                self.loading = false;
                self.download = None;
                self.set_status(err, true);
            }
            None => {}
        }

        if self
            .engine
            .as_ref()
            .is_some_and(|engine| engine.is_finished())
        {
            self.play_next();
        }

        // Poll the QR status from the UI thread so the API worker stays free.
        if let Some(dialog) = &mut self.qr {
            if !dialog.finished && dialog.last_poll.elapsed() >= net::QR_POLL_INTERVAL {
                dialog.last_poll = Instant::now();
                let key = dialog.key.clone();
                self.send(Cmd::QrPoll { key });
            }
        }

        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > TOAST_TTL)
        {
            self.toast = None;
        }

        // The settings sheet shows how much disk the cache is using. Measure on
        // the frame it opens rather than every frame, and forget it again on
        // close so reopening shows fresh numbers.
        if self.settings_open {
            if self.cache_bytes.is_none() {
                self.cache_bytes = Some(platform::dir_bytes(&self.cache_root()));
            }
        } else {
            self.cache_bytes = None;
        }
    }

    /// Switch platforms.
    ///
    /// The list, the query and the page are dropped: results from one platform are
    /// not results from the other, and the account-only tabs make no sense on
    /// YouTube, so a switch lands on search.
    pub(crate) fn set_source(&mut self, source: crate::api::models::Source) {
        if self.source == source {
            return;
        }
        self.source = source;
        self.update_config(|config| config.source = source);
        self.tab = Tab::Search;
        self.results.clear();
        self.search_page = 1;
        self.search_has_more = false;
        self.loading_more = false;
        self.searching = false;
        self.notify(match source {
            crate::api::models::Source::Bilibili => "已切换到 B 站".to_owned(),
            crate::api::models::Source::Youtube => "已切换到 YouTube".to_owned(),
        });
    }

    /// Global shortcuts: `空格` play/pause, `←`/`→` ±5s, `⌘K` focus search,
    /// `Esc` close overlays. Skipped while a text field owns the keyboard, so
    /// typing a query never toggles playback.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        // `Esc` closes overlays even while a text field owns the keyboard.
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.settings_open = false;
            self.account_menu_open = false;
            self.queue_open = false;
            if self.qr.as_ref().is_some_and(|dialog| dialog.finished) {
                self.qr = None;
            }
            return;
        }
        // Typing a query must never toggle playback or seek.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (space, left, right, focus) = ctx.input(|input| {
            (
                input.key_pressed(egui::Key::Space),
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
                input.modifiers.command && input.key_pressed(egui::Key::K),
            )
        });

        if focus {
            self.focus_search = true;
            return;
        }
        if space && self.engine.is_some() && !self.loading {
            self.toggle_pause();
        }
        if left || right {
            let delta = Duration::from_secs(5);
            let position = self.position();
            let target = if right {
                position.saturating_add(delta)
            } else {
                position.saturating_sub(delta)
            };
            let duration = self.duration();
            if !duration.is_zero() && target <= duration {
                self.try_seek(target);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.36 hands us a root `Ui` rather than a `Context`, and panels
        // are added to it. Cloning the context keeps the borrow checker happy
        // and `Context` is just a handle.
        let ctx = ui.ctx().clone();

        self.handle_events(&ctx);
        self.tick();
        self.handle_keys(&ctx);

        // Re-read the design tokens every frame: the settings sheet writes them
        // to the config, and this is the cheapest place to notice.
        self.theme = self.config_value(Theme::from_config);
        let fonts_key = self.config_value(crate::ui::theme::fonts_key);
        if self.fonts_key != fonts_key {
            let font = self.config_value(|c| {
                crate::platform::resolve_cjk_font(&c.cjk_font, c.cjk_font_path.as_deref())
            });
            crate::ui::theme::install_fonts(&ctx, font.as_deref());
            self.fonts_key = fonts_key;
        }
        let style_key = self.theme.accent.id.to_owned();
        if self.style_key != style_key {
            crate::ui::theme::install_style(&ctx, &self.theme);
            self.style_key = style_key;
        }

        // Panels are applied outermost-first: the first bottom panel added sits
        // at the very bottom of the window.
        egui::Panel::top("top_bar")
            .exact_size(crate::ui::theme::TOP_BAR_H)
            .frame(crate::ui::frames::top_bar())
            .show(ui, |ui| self.ui_top_bar(ui));
        egui::Panel::bottom("status_bar")
            .exact_size(crate::ui::theme::STATUS_BAR_H)
            .frame(crate::ui::frames::status_bar())
            .show(ui, |ui| self.ui_status_bar(ui));
        egui::Panel::bottom("player_bar")
            .frame(crate::ui::frames::player_bar(&self.theme))
            .show(ui, |ui| self.ui_player_bar(ui));
        egui::Panel::right("lyrics_panel")
            .resizable(true)
            .default_size(388.0)
            .min_size(260.0)
            .max_size(620.0)
            .frame(crate::ui::frames::lyrics(&self.theme))
            .show(ui, |ui| self.ui_lyrics_panel(ui));
        egui::CentralPanel::default()
            .frame(crate::ui::frames::list_pane())
            .show(ui, |ui| self.ui_central(ui));

        if self.qr.is_some() {
            self.ui_login_dialog(&ctx);
        }
        if self.settings_open {
            self.ui_settings_sheet(&ctx);
        }

        // Keeps the progress bar, equalizer and lyric highlight moving while
        // the app is otherwise idle.
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    //! The search paging state machine. A later page is appended to the list,
    //! so it must never trigger the skeleton or wipe what is on screen.

    use super::*;
    use crate::net::Evt;

    fn an_app() -> (App, egui::Context) {
        std::env::set_var("HOME", "/tmp/listenbli-paging-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-paging-tests");
        (
            App::new_for_tests(Config::default()),
            egui::Context::default(),
        )
    }

    /// The point of the setting: the workers actually follow it, and a location
    /// we cannot write to is refused before it is saved.
    #[test]
    fn changing_the_cache_dir_moves_the_workers_and_is_refused_when_unwritable() {
        let (mut app, _ctx) = an_app();
        let default_root = app.cache_root();
        assert_eq!(
            default_root,
            platform::cache_dir(),
            "no override means default"
        );
        assert_eq!(app.worker.cache.dir(), default_root.join("audio"));

        let chosen = std::env::temp_dir().join(format!("listenbli-moved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&chosen);
        app.set_cache_dir(Some(chosen.clone()));

        assert_eq!(
            app.config_value(|config| config.cache_dir.clone()),
            Some(chosen.clone()),
            "the choice must be persisted"
        );
        assert_eq!(app.cache_root(), chosen);
        assert_eq!(
            app.worker.cache.dir(),
            chosen.join("audio"),
            "the download worker must write to the new root"
        );
        // The root itself is created up front; `audio/` appears with the first
        // download, which is what keeps an unused cache from leaving litter.
        assert!(chosen.is_dir(), "the chosen root is created");

        // A path under a regular file can never be a directory: refuse it and
        // keep the previous choice rather than saving something unusable.
        let file = std::env::temp_dir().join(format!("listenbli-notdir-{}", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        app.set_cache_dir(Some(file.join("under")));

        assert_eq!(
            app.config_value(|config| config.cache_dir.clone()),
            Some(chosen.clone()),
            "a refused path must not replace the working one"
        );
        assert!(app.toast.is_some(), "the refusal is reported");
        assert_eq!(app.worker.cache.dir(), chosen.join("audio"));

        // Clearing it goes back to the platform default.
        app.set_cache_dir(None);
        assert_eq!(app.config_value(|config| config.cache_dir.clone()), None);
        assert_eq!(app.worker.cache.dir(), default_root.join("audio"));

        let _ = std::fs::remove_dir_all(&chosen);
        let _ = std::fs::remove_file(&file);
    }

    fn track(bvid: &str) -> Track {
        Track {
            bvid: bvid.to_owned(),
            source: crate::api::models::Source::Bilibili,
            aid: 0,
            cid: 0,
            title: format!("标题 {bvid}"),
            author: "音乐无限".to_owned(),
            duration: 200,
            cover: None,
        }
    }

    fn results(keyword: &str, page: u32, ids: &[&str], has_more: bool) -> Evt {
        Evt::SearchResults {
            keyword: keyword.to_owned(),
            page,
            tracks: ids.iter().map(|id| track(id)).collect(),
            has_more,
        }
    }

    #[test]
    fn a_later_page_is_appended_without_clearing_the_list() {
        let (mut app, ctx) = an_app();
        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "周杰伦".into(),
                page: 1,
            },
        );
        assert!(app.searching, "page 1 drives the skeleton");
        app.handle_event(&ctx, results("周杰伦", 1, &["BV1", "BV2"], true));
        assert!(!app.searching);
        assert_eq!((app.search_page, app.search_has_more), (1, true));

        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "周杰伦".into(),
                page: 2,
            },
        );
        assert!(app.loading_more, "page 2 is an append, not a fresh search");
        assert!(!app.searching, "page 2 must not show the skeleton");
        assert_eq!(app.results.len(), 2, "page 2 must not clear the list");

        app.handle_event(&ctx, results("周杰伦", 2, &["BV3"], false));
        assert_eq!(app.results.len(), 3);
        assert_eq!((app.search_page, app.search_has_more), (2, false));
        assert!(!app.loading_more);
    }

    #[test]
    fn appending_a_page_skips_results_already_listed() {
        let (mut app, ctx) = an_app();
        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "周杰伦".into(),
                page: 1,
            },
        );
        app.handle_event(&ctx, results("周杰伦", 1, &["BV1", "BV2"], true));
        // B 站第 2 页实测会重复第 1 页的部分视频。
        app.handle_event(&ctx, results("周杰伦", 2, &["BV2", "BV3"], true));

        let listed: Vec<&str> = app.results.iter().map(|t| t.bvid.as_str()).collect();
        assert_eq!(listed, ["BV1", "BV2", "BV3"]);
    }

    #[test]
    fn a_page_that_adds_nothing_ends_the_paging() {
        let (mut app, ctx) = an_app();
        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "周杰伦".into(),
                page: 1,
            },
        );
        app.handle_event(&ctx, results("周杰伦", 1, &["BV1", "BV2"], true));
        // 越界页被服务端收敛：返回的还是那两首，且声称还有下一页。
        app.handle_event(&ctx, results("周杰伦", 9, &["BV1", "BV2"], true));

        assert_eq!(app.results.len(), 2);
        assert!(
            !app.search_has_more,
            "a page with nothing new means the list is complete"
        );
    }

    #[test]
    fn a_new_query_restarts_at_page_one() {
        let (mut app, ctx) = an_app();
        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "周杰伦".into(),
                page: 1,
            },
        );
        app.handle_event(&ctx, results("周杰伦", 3, &["BV1"], true));
        assert_eq!(app.results.len(), 1);
        assert_eq!((app.search_page, app.search_has_more), (3, true));

        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "米津玄師".into(),
                page: 1,
            },
        );
        assert!(
            app.results.is_empty(),
            "a new query starts from a clean list"
        );
        assert_eq!((app.search_page, app.search_has_more), (1, false));

        app.handle_event(&ctx, results("米津玄師", 1, &["BV9"], true));
        assert_eq!(app.results.len(), 1);
        assert!(app.search_has_more);
    }

    #[test]
    fn a_late_page_for_a_replaced_query_is_ignored() {
        let (mut app, ctx) = an_app();
        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "新查询".into(),
                page: 1,
            },
        );

        // The user typed a new keyword while page 2 of the old one was in flight.
        app.handle_event(&ctx, results("旧查询", 2, &["BV1"], true));
        assert!(
            app.results.is_empty(),
            "an outdated page must not be appended"
        );
        assert_eq!(app.search_page, 1);
        assert!(!app.search_has_more);
    }

    #[test]
    fn a_failed_page_stops_the_spinner() {
        let (mut app, ctx) = an_app();
        app.handle_event(
            &ctx,
            Evt::SearchStarted {
                keyword: "周杰伦".into(),
                page: 2,
            },
        );
        assert!(app.loading_more);

        app.handle_event(
            &ctx,
            Evt::Error {
                context: "搜索失败".into(),
                message: "网络错误".into(),
            },
        );
        assert!(!app.loading_more, "the button must not spin forever");
        assert!(!app.searching);
    }

    /// The row menu's queue entries: append at the end, or jump the line.
    #[test]
    fn enqueue_appends_and_refuses_duplicates() {
        let (mut app, _ctx) = an_app();
        assert!(app.enqueue(track("BV1"), false));
        assert!(app.enqueue(track("BV2"), false));
        assert_eq!(
            app.queue
                .iter()
                .map(|t| t.bvid.as_str())
                .collect::<Vec<_>>(),
            ["BV1", "BV2"]
        );

        assert!(!app.enqueue(track("BV1"), false), "同一首不该排队两次");
        assert_eq!(app.queue.len(), 2);
    }

    #[test]
    fn enqueue_next_lands_right_after_the_current_track() {
        let (mut app, _ctx) = an_app();
        app.queue = vec![track("BV1"), track("BV3")];
        app.queue_pos = 0;

        assert!(app.enqueue(track("BV2"), true));

        assert_eq!(
            app.queue
                .iter()
                .map(|t| t.bvid.as_str())
                .collect::<Vec<_>>(),
            ["BV1", "BV2", "BV3"]
        );
    }

    /// A cache-only request must never point a second writer at the file the
    /// player is already streaming into.
    #[test]
    fn caching_is_refused_while_that_track_is_streaming() {
        let (mut app, _ctx) = an_app();
        let streaming = track("BV1");
        assert!(
            app.cache_track(&streaming),
            "idle tracks should be accepted"
        );

        app.download = Some((streaming.key(), 128, Some(256)));
        assert!(
            !app.cache_track(&streaming),
            "the streaming file must not get a second writer"
        );
        assert!(
            app.cache_track(&track("BV2")),
            "another track is unaffected"
        );
    }
}

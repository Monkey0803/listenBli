//! Background workers and the command/event protocol between them and the UI.
//!
//! Four threads exist behind the UI:
//! * the UI thread (eframe), which never blocks;
//! * an **API worker** that resolves tracks, drives login and loads library
//!   pages — everything a click waits on;
//! * a **cover worker**, a **lyrics worker**, and
//! * a **download worker** that streams audio segments with progress.
//!
//! The first three are separate on purpose. A search asks for one cover per
//! result, and a cover is an HTTP request plus a decode; doing that on the API
//! worker used to put a click behind ~20 images, which read as "loading takes
//! ages" even though resolving and streaming a track takes well under a second.
//! The download worker is likewise independent, and a shared "current request"
//! token lets it abandon a segment as soon as the user picks something else.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::ColorImage;

use crate::api::client::{Api, ApiError, BILI_WEB};
use crate::api::models::{AudioQuality, FavFolder, Track, UserInfo};
use crate::api::video::AudioSource;
use crate::api::{library, login, search, video};
use crate::audio::cache::looks_like_iso_bmff;
use crate::audio::{self, AudioCache, PlaybackSource, StreamState};
use crate::config::{self, SharedConfig};
use crate::lyrics::{self, Lyrics};
use crate::platform;
use crate::util;

/// Progress events are throttled to this granularity to avoid flooding the UI.
const PROGRESS_STEP_BYTES: u64 = 128 * 1024;

/// Hand playback to the UI once this much of the segment has landed.
///
/// Deliberately small: the decoder's reads block until data arrives, so it only
/// needs the init segment to start making progress. Waiting for more would just
/// delay the first note for no benefit.
const ANNOUNCE_AFTER_BYTES: u64 = 64 * 1024;

#[derive(Debug)]
pub enum Cmd {
    Search {
        keyword: String,
        page: u32,
    },
    /// Resolve, download (if not cached) and hand back a playable file.
    LoadTrack(Box<Track>),
    /// Download into the cache without touching playback.
    CacheTrack(Box<Track>),
    /// Write a normal `.m4a` copy of the track's audio into the export folder.
    ExportTrack(Box<Track>),
    QrStart,
    QrPoll {
        key: String,
    },
    RefreshLogin,
    Logout,
    LoadFavFolders {
        mid: i64,
    },
    LoadFavItems {
        media_id: i64,
        page: u32,
    },
    LoadHistory,
}

#[derive(Debug)]
pub enum Evt {
    SearchStarted {
        keyword: String,
        page: u32,
    },
    SearchResults {
        keyword: String,
        page: u32,
        tracks: Vec<Track>,
        /// Whether the server says another page can be requested.
        has_more: bool,
    },
    /// The track's `cid`/duration are now known; the download may still be running.
    TrackResolved(Box<Track>),
    TrackReady {
        track: Box<Track>,
        /// Either a fully cached file, or a segment that is still arriving.
        source: PlaybackSource,
        quality: AudioQuality,
    },
    DownloadProgress {
        key: String,
        got: u64,
        total: Option<u64>,
    },
    /// A cache-only download finished (or found the track already cached).
    Cached {
        title: String,
        already: bool,
    },
    /// An exported `.m4a` is on disk.
    Exported {
        title: String,
        path: PathBuf,
    },
    LyricsReady {
        key: String,
        lyrics: Lyrics,
    },
    CoverReady {
        key: String,
        image: ColorImage,
    },
    QrCode {
        key: String,
        image: ColorImage,
    },
    /// Human readable status for the login dialog; `done` closes it.
    LoginStatus {
        message: String,
        done: bool,
    },
    LoginChanged {
        user: Option<UserInfo>,
    },
    FavFolders(Vec<FavFolder>),
    FavItems {
        media_id: i64,
        page: u32,
        tracks: Vec<Track>,
        has_more: bool,
    },
    History(Vec<Track>),
    Error {
        context: String,
        message: String,
    },
    Info(String),
}

pub struct Worker {
    pub cmd_tx: Sender<Cmd>,
    /// Cover art, on its own worker so it never delays playback.
    pub cover_tx: Sender<CoverJob>,
    pub evt_rx: Receiver<Evt>,
    pub api: Arc<Api>,
    /// Shared with the download worker, so the UI can point it at a new root the
    /// moment the user changes the setting.
    pub cache: Arc<AudioCache>,
}

struct DownloadJob {
    track: Track,
    source: AudioSource,
    /// Whether the UI wants to *play* this track once bytes are on disk. A
    /// cache-only job just fills the cache and reports back.
    play: bool,
    /// Which `LoadTrack` request this job belongs to; a job is abandoned as soon
    /// as a newer request arrives. Cache-only jobs carry `None`: they must never
    /// supersede a playback request, and playback must not cancel them.
    generation: Option<u64>,
    /// Write an `.m4a` once the bytes are in the cache. Only set for an export,
    /// which is why it is not part of `play`.
    export: bool,
}

/// Where the API worker fans its results out to.
struct Sinks {
    events: Sender<Evt>,
    downloads: Sender<DownloadJob>,
    lyrics: Sender<Box<Track>>,
}

/// One cover to fetch. Decorative, so it never shares a queue with playback.
#[derive(Debug)]
pub struct CoverJob {
    pub key: String,
    pub url: String,
}

/// Start the workers. `api` is shared with the caller so the UI can read the
/// cookie jar when it needs to.
pub fn spawn(api: Arc<Api>, config: SharedConfig) -> Worker {
    let (cmd_tx, cmd_rx) = unbounded::<Cmd>();
    let (cover_tx, cover_rx) = unbounded::<CoverJob>();
    let (lyrics_tx, lyrics_rx) = unbounded::<Box<Track>>();
    let (evt_tx, evt_rx) = unbounded::<Evt>();
    let (job_tx, job_rx) = unbounded::<DownloadJob>();
    // A monotonic request counter rather than the track's `cid`: re-clicking the
    // same song must also supersede the previous download, otherwise two threads
    // would write the same cache file at once.
    let request_id = Arc::new(AtomicU64::new(0));
    let cache = Arc::new(AudioCache::new());

    {
        let api = Arc::clone(&api);
        let config = Arc::clone(&config);
        let request_id = Arc::clone(&request_id);
        let cache = Arc::clone(&cache);
        let sinks = Sinks {
            events: evt_tx.clone(),
            downloads: job_tx,
            lyrics: lyrics_tx,
        };
        let _ = std::thread::Builder::new()
            .name("listenbli-api".into())
            .spawn(move || api_loop(api, config, cmd_rx, sinks, request_id, cache));
    }

    {
        let api = Arc::clone(&api);
        let evt_tx = evt_tx.clone();
        let _ = std::thread::Builder::new()
            .name("listenbli-cover".into())
            .spawn(move || cover_loop(api, cover_rx, evt_tx));
    }

    {
        let api = Arc::clone(&api);
        let config = Arc::clone(&config);
        let evt_tx = evt_tx.clone();
        let _ = std::thread::Builder::new()
            .name("listenbli-lyrics".into())
            .spawn(move || lyrics_loop(api, config, lyrics_rx, evt_tx));
    }

    {
        let api = Arc::clone(&api);
        let evt_tx = evt_tx.clone();
        let cache = Arc::clone(&cache);
        let _ = std::thread::Builder::new()
            .name("listenbli-download".into())
            .spawn(move || download_loop(api, job_rx, evt_tx, request_id, cache));
    }

    // A cache built before the app stored playable files holds fragments only, and
    // nothing outside this app can play those. Upgrade them in the background,
    // once per run, skipping whatever is still being downloaded.
    {
        let cache = Arc::clone(&cache);
        let evt_tx = evt_tx.clone();
        let _ = std::thread::Builder::new()
            .name("listenbli-convert".into())
            .spawn(move || {
                let converted = cache.convert_finished();
                if converted > 0 {
                    let _ = evt_tx.send(Evt::Info(format!(
                        "已把 {converted} 首缓存转成可播放的 m4a"
                    )));
                }
            });
    }

    Worker {
        cmd_tx,
        cover_tx,
        evt_rx,
        api,
        cache,
    }
}

impl Worker {
    /// Queue one cover fetch. Decorative work never shares a queue with anything
    /// the user is waiting for, so this bypasses `cmd_tx` on purpose.
    pub fn fetch_cover(&self, key: String, url: String) {
        let _ = self.cover_tx.send(CoverJob { key, url });
    }
}

// ---------------------------------------------------------------------------
// API worker
// ---------------------------------------------------------------------------

fn api_loop(
    api: Arc<Api>,
    config: SharedConfig,
    cmd_rx: Receiver<Cmd>,
    sinks: Sinks,
    request_id: Arc<AtomicU64>,
    cache: Arc<AudioCache>,
) {
    // Best-effort anonymous fingerprint; failures are not fatal.
    if let Err(err) = api.bootstrap_fingerprint() {
        eprintln!("fingerprint bootstrap failed: {err}");
    }

    while let Ok(cmd) = cmd_rx.recv() {
        dispatch(&api, &config, cmd, &sinks, &request_id, &cache);
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    api: &Api,
    config: &SharedConfig,
    cmd: Cmd,
    sinks: &Sinks,
    request_id: &Arc<AtomicU64>,
    cache: &Arc<AudioCache>,
) {
    let evt_tx = &sinks.events;
    match cmd {
        Cmd::Search { keyword, page } => {
            let _ = evt_tx.send(Evt::SearchStarted {
                keyword: keyword.clone(),
                page,
            });
            match search::search(api, &keyword, page) {
                Ok(results) => {
                    let _ = evt_tx.send(Evt::SearchResults {
                        keyword,
                        page,
                        tracks: results.tracks,
                        has_more: results.has_more,
                    });
                }
                Err(err) => send_error(evt_tx, "搜索失败", err),
            }
        }

        Cmd::LoadTrack(track) => {
            load_track(api, config, *track, sinks, request_id, cache, Purpose::Play);
        }

        Cmd::CacheTrack(track) => {
            load_track(
                api,
                config,
                *track,
                sinks,
                request_id,
                cache,
                Purpose::Cache,
            );
        }
        Cmd::ExportTrack(track) => {
            load_track(
                api,
                config,
                *track,
                sinks,
                request_id,
                cache,
                Purpose::Export,
            );
        }

        Cmd::QrStart => match login::generate(api) {
            Ok((key, url)) => match qr_image(&url) {
                Some(image) => {
                    let _ = evt_tx.send(Evt::QrCode { key, image });
                }
                None => {
                    let _ = evt_tx.send(Evt::LoginStatus {
                        message: "二维码生成失败".into(),
                        done: true,
                    });
                }
            },
            Err(err) => send_error(evt_tx, "获取登录二维码失败", err),
        },

        Cmd::QrPoll { key } => match login::poll(api, &key) {
            Ok((status, cross_domain_url)) => {
                handle_qr_status(api, config, status, cross_domain_url, evt_tx)
            }
            Err(err) => send_error(evt_tx, "登录状态查询失败", err),
        },

        Cmd::RefreshLogin => {
            let user = refresh_login(api, config, evt_tx);
            let _ = evt_tx.send(Evt::LoginChanged { user });
        }

        Cmd::Logout => {
            api.replace_jar(Default::default());
            {
                let mut guard = config.lock().unwrap();
                guard.cookies = Default::default();
                if let Err(err) = guard.save() {
                    eprintln!("saving config after logout failed: {err}");
                }
            }
            let _ = evt_tx.send(Evt::LoginChanged { user: None });
            let _ = evt_tx.send(Evt::Info("已退出登录".into()));
        }

        Cmd::LoadFavFolders { mid } => match library::fav_folders(api, mid) {
            Ok(folders) => {
                let _ = evt_tx.send(Evt::FavFolders(folders));
            }
            Err(err) => send_error(evt_tx, "加载收藏夹失败", err),
        },

        Cmd::LoadFavItems { media_id, page } => match library::fav_items(api, media_id, page) {
            Ok((tracks, has_more)) => {
                let _ = evt_tx.send(Evt::FavItems {
                    media_id,
                    page,
                    tracks,
                    has_more,
                });
            }
            Err(err) => send_error(evt_tx, "加载收藏内容失败", err),
        },

        Cmd::LoadHistory => match library::history(api, 0, 0) {
            Ok(tracks) => {
                let _ = evt_tx.send(Evt::History(tracks));
            }
            Err(err) => send_error(evt_tx, "加载历史记录失败", err),
        },
    }
}

fn handle_qr_status(
    api: &Api,
    config: &SharedConfig,
    status: login::QrStatus,
    cross_domain_url: Option<String>,
    evt_tx: &Sender<Evt>,
) {
    use login::QrStatus;

    match status {
        QrStatus::Pending => {
            let _ = evt_tx.send(Evt::LoginStatus {
                message: "请使用哔哩哔哩手机客户端扫码".into(),
                done: false,
            });
        }
        QrStatus::Scanned => {
            let _ = evt_tx.send(Evt::LoginStatus {
                message: "已扫码，请在手机上确认登录".into(),
                done: false,
            });
        }
        QrStatus::Expired => {
            let _ = evt_tx.send(Evt::LoginStatus {
                message: "二维码已过期".into(),
                done: true,
            });
        }
        QrStatus::Unknown(code, message) => {
            let _ = evt_tx.send(Evt::LoginStatus {
                message: format!("登录异常 ({code}) {message}"),
                done: true,
            });
        }
        QrStatus::Confirmed => {
            // Normally `Set-Cookie` already populated the jar. If not, the
            // cross-domain URL repeats the credentials as query parameters.
            if !api.has_login_cookie() {
                if let Some(url) = cross_domain_url {
                    let mut jar = api.jar_snapshot();
                    for (name, value) in login::cookies_from_cross_domain_url(&url) {
                        jar.set("bilibili.com", &name, &value);
                    }
                    api.replace_jar(jar);
                }
            }

            let user = refresh_login(api, config, evt_tx);
            let logged_in = user.is_some();
            let _ = evt_tx.send(Evt::LoginStatus {
                message: if logged_in {
                    "登录成功".into()
                } else {
                    "登录失败：未能取得登录凭据".into()
                },
                done: true,
            });
            let _ = evt_tx.send(Evt::LoginChanged { user });
        }
    }
}

/// Re-read `nav` and persist the cookie jar alongside the derived user info.
fn refresh_login(api: &Api, config: &SharedConfig, evt_tx: &Sender<Evt>) -> Option<UserInfo> {
    match api.nav() {
        Ok((user, is_login)) => {
            // Persist whatever the jar now holds, logged in or not.
            {
                let mut guard = config.lock().unwrap();
                guard.cookies = api.jar_snapshot();
                if let Err(err) = guard.save() {
                    eprintln!("saving config failed: {err}");
                }
            }
            if is_login {
                Some(user)
            } else {
                let _ = evt_tx.send(Evt::Info("当前为未登录状态，音质与「我的」功能受限".into()));
                None
            }
        }
        Err(err) => {
            send_error(evt_tx, "获取账号信息失败", err);
            None
        }
    }
}

/// What a resolved track is for. Two bare booleans at the call sites read as
/// nothing in particular; this says it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    /// Play it (and keep the cache warm while doing so).
    Play,
    /// Fill the cache and say so.
    Cache,
    /// Fill the cache if needed, then write a normal `.m4a` next to it.
    Export,
}

impl Purpose {
    fn plays(self) -> bool {
        self == Purpose::Play
    }
}

fn load_track(
    api: &Api,
    config: &SharedConfig,
    mut track: Track,
    sinks: &Sinks,
    request_id: &Arc<AtomicU64>,
    cache: &Arc<AudioCache>,
    purpose: Purpose,
) {
    let play = purpose.plays();
    let evt_tx = &sinks.events;
    let job_tx = &sinks.downloads;
    let lyrics_tx = &sinks.lyrics;
    if let Err(err) = video::resolve_track(api, &mut track) {
        send_error(evt_tx, "获取视频信息失败", err);
        return;
    }
    if track.cid == 0 {
        let _ = evt_tx.send(Evt::Error {
            context: "播放失败".into(),
            message: "该视频没有可播放的音频流".into(),
        });
        return;
    }

    let key = track.key();
    if play {
        // Cache-only jobs must not disturb what the list shows as playing.
        let _ = evt_tx.send(Evt::TrackResolved(Box::new(track.clone())));
    }

    let prefer_flac = config.lock().unwrap().prefer_flac;
    let playurl = match video::playurl(api, &track.bvid, track.cid) {
        Ok(data) => data,
        Err(err) => {
            send_error(evt_tx, "获取音频地址失败", err);
            return;
        }
    };

    let Some(source) = video::pick_audio(&playurl, prefer_flac) else {
        let _ = evt_tx.send(Evt::Error {
            context: "播放失败".into(),
            message: format!("没有可用的音频流（{}）", video::describe_streams(&playurl)),
        });
        return;
    };

    // `playurl.timelength` is authoritative: the decoded fMP4 reports no
    // duration of its own (verified in the M0 spike).
    let duration_secs = playurl
        .timelength
        .map(|ms| ms / 1000)
        .filter(|secs| *secs > 0)
        .unwrap_or(track.duration);
    if duration_secs > 0 {
        track.duration = duration_secs;
    }

    // Newest request wins; lets the download worker abandon older segments.
    // A generation counter rather than the cid, so re-clicking the same song
    // also supersedes its earlier download. Cache-only jobs stay out of that
    // race in both directions.
    let generation = play.then(|| request_id.fetch_add(1, Ordering::SeqCst) + 1);

    if let Some(path) = cache.get(track.cid, source.quality) {
        if play {
            // Fully cached: plain, non-blocking playback.
            let _ = evt_tx.send(Evt::TrackReady {
                track: Box::new(track.clone()),
                source: PlaybackSource::Complete(path),
                quality: source.quality,
            });
        } else if purpose == Purpose::Export {
            spawn_export(cache.clone(), track.clone(), source.quality, evt_tx.clone());
        } else {
            let _ = evt_tx.send(Evt::Cached {
                title: track.title.clone(),
                already: true,
            });
        }
    } else {
        let _ = evt_tx.send(Evt::DownloadProgress {
            key: key.clone(),
            got: 0,
            total: None,
        });
        let _ = job_tx.send(DownloadJob {
            track: track.clone(),
            source,
            play,
            generation,
            export: purpose == Purpose::Export,
        });
    }

    // Lyrics are looked up on their own thread: they must not delay the next
    // click, and they must not queue behind a screen full of cover art. A
    // cache-only job has nothing to show them in.
    if play {
        let _ = lyrics_tx.send(Box::new(track));
    }
}

fn send_error(evt_tx: &Sender<Evt>, context: &str, err: ApiError) {
    let _ = evt_tx.send(Evt::Error {
        context: context.to_string(),
        message: err.to_string(),
    });
}

// ---------------------------------------------------------------------------
// Cover worker
// ---------------------------------------------------------------------------

fn cover_loop(api: Arc<Api>, cover_rx: Receiver<CoverJob>, evt_tx: Sender<Evt>) {
    while let Ok(job) = cover_rx.recv() {
        // Bilibili can resize on the fly; fall back to the full image when the
        // resized variant is unavailable.
        let thumb = util::cover_thumbnail_url(&job.url, 120);
        let image = fetch_cover(&api, &thumb).or_else(|_| fetch_cover(&api, &job.url));
        match image {
            Ok(image) => {
                let _ = evt_tx.send(Evt::CoverReady {
                    key: job.key,
                    image,
                });
            }
            // Covers are decorative: log and move on.
            Err(err) => eprintln!("cover {} failed: {err}", job.url),
        }
    }
}

// ---------------------------------------------------------------------------
// Lyrics worker
// ---------------------------------------------------------------------------

fn lyrics_loop(
    api: Arc<Api>,
    config: SharedConfig,
    lyrics_rx: Receiver<Box<Track>>,
    evt_tx: Sender<Evt>,
) {
    while let Ok(track) = lyrics_rx.recv() {
        let key = track.key();
        // Read per track rather than once: the user may move the cache while the
        // app is running, and the lyric cache has to follow it.
        let root = config::with(&config, |config| {
            platform::resolve_cache_dir(config.cache_dir.as_deref())
        });
        let lyrics = lyrics::fetch_for(&api, &track, &root);
        // A stale answer is harmless: the UI matches on the track key.
        let _ = evt_tx.send(Evt::LyricsReady { key, lyrics });
    }
}

// ---------------------------------------------------------------------------
// Download worker
// ---------------------------------------------------------------------------

/// Remux a cached segment into a normal audio file on its own thread.
///
/// The copy is hundreds of megabytes and must not stall the worker that serves
/// playback, so it gets a thread rather than a slot in the queue.
fn spawn_export(cache: Arc<AudioCache>, track: Track, quality: AudioQuality, evt_tx: Sender<Evt>) {
    let title = track.title.clone();
    let _ = std::thread::Builder::new()
        .name("listenbli-export".into())
        .spawn(move || {
            let Some(segment) = cache.get(track.cid, quality) else {
                let _ = evt_tx.send(Evt::Error {
                    context: format!("导出失败：{title}"),
                    message: "缓存里找不到这段音频".to_owned(),
                });
                return;
            };
            let out = platform::export_dir().join(audio::export::file_name(&title, &track.author));
            match audio::export::export_any(&segment, &out) {
                Ok(()) => {
                    let _ = evt_tx.send(Evt::Exported { title, path: out });
                }
                Err(message) => {
                    let _ = evt_tx.send(Evt::Error {
                        context: format!("导出失败：{title}"),
                        message,
                    });
                }
            }
        });
}

fn download_loop(
    api: Arc<Api>,
    job_rx: Receiver<DownloadJob>,
    evt_tx: Sender<Evt>,
    request_id: Arc<AtomicU64>,
    cache: Arc<AudioCache>,
) {
    while let Ok(job) = job_rx.recv() {
        // Skip jobs that have already been superseded while queued.
        if is_superseded(&request_id, job.generation) {
            continue;
        }
        if let Err(message) = run_download(&api, &job, &evt_tx, &request_id, &cache) {
            if message == SUPERSEDED {
                continue;
            }
            let _ = evt_tx.send(Evt::Error {
                context: format!("下载音频失败：{}", job.track.title),
                message,
            });
        }
    }
}

/// Sentinel for "a newer request replaced this one"; not a user-facing failure.
const SUPERSEDED: &str = "__superseded__";

fn is_superseded(request_id: &AtomicU64, generation: Option<u64>) -> bool {
    generation.is_some_and(|generation| request_id.load(Ordering::SeqCst) != generation)
}

/// True once the file's first bytes look like an ISO-BMFF container.
///
/// The CDN answers some failures with an HTML error page under a 200 status, so
/// this check is what turns "silent garbage" into a clear error.
fn head_is_iso_bmff(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 16];
    match file.read(&mut head) {
        Ok(read) => looks_like_iso_bmff(&head[..read]),
        Err(_) => false,
    }
}

fn run_download(
    api: &Api,
    job: &DownloadJob,
    evt_tx: &Sender<Evt>,
    request_id: &AtomicU64,
    cache: &Arc<AudioCache>,
) -> Result<(), String> {
    let key = job.track.key();
    let mut last_err = String::from("没有可用的下载地址");

    // Primary URL first, then the CDN backups. The first attempt covers the
    // common case; a second only happens when the primary is rejected.
    for url in job.source.urls.iter().take(2) {
        match stream_segment(api, url, job, evt_tx, request_id, cache) {
            Ok(()) => {
                // Opportunistic housekeeping; never blocks playback.
                let _ = cache.evict_to_fit();
                return Ok(());
            }
            Err(err) if err == SUPERSEDED => return Err(SUPERSEDED.to_string()),
            Err(err) => {
                eprintln!("segment download from {url} failed: {err}");
                last_err = err;
            }
        }
    }

    let _ = evt_tx.send(Evt::DownloadProgress {
        key,
        got: 0,
        total: None,
    });
    Err(last_err)
}

/// Download straight into the cache file while the decoder reads along.
///
/// The decoder is told the segment's real size and its reads block until bytes
/// land, so playback starts as soon as [`ANNOUNCE_AFTER_BYTES`] are present
/// rather than after the whole song. The cache sidecar records the expected
/// size so a partial file is never later mistaken for a complete one.
fn stream_segment(
    api: &Api,
    url: &str,
    job: &DownloadJob,
    evt_tx: &Sender<Evt>,
    request_id: &AtomicU64,
    cache: &Arc<AudioCache>,
) -> Result<(), String> {
    let key = job.track.key();
    let cid = job.track.cid;
    let quality = job.source.quality;

    let mut response = api
        .get_stream(url, Some(BILI_WEB))
        .map_err(|e| e.to_string())?;
    let total = response
        .content_length()
        .ok_or_else(|| "CDN 未返回 Content-Length，无法流式播放".to_string())?;
    if total == 0 {
        return Err("CDN 返回了空内容".to_string());
    }

    platform::ensure_dir(&cache.dir()).map_err(|e| format!("创建缓存目录失败: {e}"))?;
    let path = cache.path_for(cid, quality);

    // Fresh attempt: drop any stale partial data and record what to expect.
    // The sidecar is written first, so a `.m4s` without one is never a hit.
    cache.remove(cid, quality);
    cache
        .begin(cid, quality, total)
        .map_err(|e| format!("写入缓存元数据失败: {e}"))?;
    let mut file = std::fs::File::create(&path).map_err(|e| format!("创建缓存文件失败: {e}"))?;

    let state = Arc::new(StreamState::new(total));

    // Any failure must also fail the stream state, otherwise a decoder already
    // reading this file would block until the read timeout.
    let abort = |message: String| -> String {
        state.fail(message.clone());
        cache.remove(cid, quality);
        message
    };

    let mut chunk = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    let mut announced = false;
    let mut last_reported = 0u64;

    loop {
        if is_superseded(request_id, job.generation) {
            state.cancel();
            cache.remove(cid, quality);
            return Err(SUPERSEDED.to_string());
        }

        let read = match response.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) => return Err(abort(format!("读取响应失败: {err}"))),
        };

        if let Err(err) = file.write_all(&chunk[..read]) {
            return Err(abort(format!("写入缓存失败: {err}")));
        }
        written += read as u64;
        // Publish only after the bytes have reached the OS, so a reader can
        // never observe data that is not yet readable.
        state.publish(written);

        if job.play && !announced && written >= ANNOUNCE_AFTER_BYTES.min(total) {
            if !head_is_iso_bmff(&path) {
                return Err(abort(
                    "CDN 返回的不是有效音频数据（可能已过期或被拦截）".to_string(),
                ));
            }
            announced = true;
            announce(evt_tx, job, &path, total, &state, quality);
        }

        if written - last_reported >= PROGRESS_STEP_BYTES {
            last_reported = written;
            let _ = evt_tx.send(Evt::DownloadProgress {
                key: key.clone(),
                got: written,
                total: Some(total),
            });
        }
    }

    if written != total {
        return Err(abort(format!(
            "下载不完整：期望 {total} 字节，实际 {written} 字节"
        )));
    }

    // Very short segments may never reach the announce threshold. A cache-only
    // job never announced, so its bytes are validated here.
    if !announced {
        if !head_is_iso_bmff(&path) {
            return Err(abort(
                "CDN 返回的不是有效音频数据（可能已过期或被拦截）".to_string(),
            ));
        }
        if job.play {
            announce(evt_tx, job, &path, total, &state, quality);
        }
    }

    let _ = file.sync_all();
    // Everything is on disk: readers now see a genuine end-of-stream.
    state.finish();
    let _ = evt_tx.send(Evt::DownloadProgress {
        key,
        got: written,
        total: Some(total),
    });
    // The bytes are whole: turn them into a file the rest of the world can play.
    // A failure here is not a failed download — the fragment still plays — so it
    // is reported as information, not as an error.
    if let Err(message) = cache.finish_download(cid, quality) {
        let _ = evt_tx.send(Evt::Info(format!("缓存转换失败，仍保留分片：{message}")));
    }

    if job.export {
        spawn_export(
            Arc::clone(cache),
            job.track.clone(),
            job.source.quality,
            evt_tx.clone(),
        );
    } else if !job.play {
        let _ = evt_tx.send(Evt::Cached {
            title: job.track.title.clone(),
            already: false,
        });
    }
    Ok(())
}

fn announce(
    evt_tx: &Sender<Evt>,
    job: &DownloadJob,
    path: &Path,
    total: u64,
    state: &Arc<StreamState>,
    quality: AudioQuality,
) {
    let _ = evt_tx.send(Evt::TrackReady {
        track: Box::new(job.track.clone()),
        source: PlaybackSource::Streaming {
            path: path.to_path_buf(),
            total,
            state: Arc::clone(state),
        },
        quality,
    });
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

/// Decode a JPEG/PNG/WebP cover and shrink it to a UI-sized thumbnail.
fn fetch_cover(api: &Api, url: &str) -> Result<ColorImage, String> {
    let bytes = api.get_bytes(url).map_err(|e| e.to_string())?;
    decode_thumbnail(&bytes, 192)
}

fn decode_thumbnail(bytes: &[u8], size: u32) -> Result<ColorImage, String> {
    let image = image::load_from_memory(bytes).map_err(|e| format!("图片解码失败: {e}"))?;
    let thumb = image.thumbnail(size, size).to_rgba8();
    let (width, height) = thumb.dimensions();
    Ok(ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        thumb.as_raw(),
    ))
}

/// Render a QR code as an egui texture, with a 2-module quiet zone.
pub fn qr_image(payload: &str) -> Option<ColorImage> {
    let code = qrcode::QrCode::new(payload.as_bytes()).ok()?;
    let width = code.width();
    let colors = code.to_colors();

    const QUIET: usize = 2;
    const MODULE_PX: usize = 5;
    let modules = width + QUIET * 2;
    let side = modules * MODULE_PX;

    let mut pixels = vec![egui::Color32::WHITE; side * side];
    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] != qrcode::Color::Dark {
                continue;
            }
            for dy in 0..MODULE_PX {
                for dx in 0..MODULE_PX {
                    let px = (x + QUIET) * MODULE_PX + dx;
                    let py = (y + QUIET) * MODULE_PX + dy;
                    pixels[py * side + px] = egui::Color32::BLACK;
                }
            }
        }
    }
    Some(ColorImage::new([side, side], pixels))
}

/// Format a byte count for the progress label.
pub fn human_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Small helper so the UI can poll the QR status on a schedule.
pub const QR_POLL_INTERVAL: Duration = Duration::from_millis(1500);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_image_is_square_and_has_dark_modules() {
        let image = qr_image("https://example.com/scan?k=abc").expect("qr should render");
        assert_eq!(image.size[0], image.size[1]);
        assert!(image.pixels.contains(&egui::Color32::BLACK));
        assert!(image.pixels.contains(&egui::Color32::WHITE));
    }

    #[test]
    fn a_job_is_superseded_by_any_newer_request() {
        let request_id = AtomicU64::new(1);
        assert!(!is_superseded(&request_id, Some(1)));
        // A newer request (including one for the *same* song) invalidates it.
        request_id.store(2, Ordering::SeqCst);
        assert!(is_superseded(&request_id, Some(1)));
        assert!(!is_superseded(&request_id, Some(2)));
    }

    /// Cache-only jobs carry no generation: playing another song must not cancel
    /// them, and asking for one must not cancel the song that is playing.
    #[test]
    fn a_cache_only_job_is_never_superseded() {
        let request_id = AtomicU64::new(7);
        assert!(!is_superseded(&request_id, None));
        request_id.store(9, Ordering::SeqCst);
        assert!(!is_superseded(&request_id, None));
    }

    #[test]
    fn human_bytes_scales() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}

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
use crate::api::models::{AudioQuality, FavFolder, Source, Track, UserInfo};
use crate::api::video::AudioSource;
use crate::api::Youtube;
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
        source: Source,
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
    /// The YouTube source, kept here so a caller can search it without the worker
    /// having to be told which platform each request is for.
    pub youtube: Arc<Youtube>,
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
    /// How to fetch the bytes: one GET, or ranges.
    plan: DownloadPlan,
    /// The stream's length, when the API stated it (a ranged source must, because
    /// a 206 response's `Content-Length` is only the chunk's own length).
    total: Option<u64>,
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
    // One YouTube client for the worker: it owns the page cursors, so it cannot be
    // rebuilt per request.
    let youtube = Arc::new(Youtube::new());

    {
        let api = Arc::clone(&api);
        let youtube = Arc::clone(&youtube);
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
            .spawn(move || api_loop(api, youtube, config, cmd_rx, sinks, request_id, cache));
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
        let youtube = Arc::clone(&youtube);
        let config = Arc::clone(&config);
        let evt_tx = evt_tx.clone();
        let _ = std::thread::Builder::new()
            .name("listenbli-lyrics".into())
            .spawn(move || lyrics_loop(api, youtube, config, lyrics_rx, evt_tx));
    }

    {
        let api = Arc::clone(&api);
        let youtube = Arc::clone(&youtube);
        let config = Arc::clone(&config);
        let evt_tx = evt_tx.clone();
        let cache = Arc::clone(&cache);
        let _ = std::thread::Builder::new()
            .name("listenbli-download".into())
            .spawn(move || download_loop(api, youtube, config, job_rx, evt_tx, request_id, cache));
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
        youtube,
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
    youtube: Arc<Youtube>,
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
        dispatch(
            &Sources {
                api: &api,
                youtube: &youtube,
                config: &config,
            },
            cmd,
            &sinks,
            &request_id,
            &cache,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    sources: &Sources<'_>,
    cmd: Cmd,
    sinks: &Sinks,
    request_id: &Arc<AtomicU64>,
    cache: &Arc<AudioCache>,
) {
    let api = sources.api;
    let config = sources.config;
    let evt_tx = &sinks.events;
    match cmd {
        Cmd::Search {
            source,
            keyword,
            page,
        } => {
            let _ = evt_tx.send(Evt::SearchStarted {
                keyword: keyword.clone(),
                page,
            });
            // Two clients, one place that decides which: the caller says what it
            // is searching, not how.
            let outcome = match source {
                Source::Bilibili => search::search(api, &keyword, page)
                    .map(|results| (results.tracks, results.has_more)),
                Source::Youtube => sources
                    .youtube
                    .search(&keyword, page)
                    .map_err(|err| ApiError::Network(err.to_string())),
            };
            match outcome {
                Ok((tracks, has_more)) => {
                    let _ = evt_tx.send(Evt::SearchResults {
                        keyword,
                        page,
                        tracks,
                        has_more,
                    });
                }
                Err(err) => send_error(evt_tx, "搜索失败", err),
            }
        }

        Cmd::LoadTrack(track) => {
            load_track(sources, *track, sinks, request_id, cache, Purpose::Play);
        }

        Cmd::CacheTrack(track) => {
            load_track(sources, *track, sinks, request_id, cache, Purpose::Cache);
        }
        Cmd::ExportTrack(track) => {
            load_track(sources, *track, sinks, request_id, cache, Purpose::Export);
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

/// The clients a load needs: both platforms, and the settings that affect
/// resolution.
///
/// Bundled because they travel together through every resolution step, and because
/// passing them separately pushed `load_track` past eight arguments.
struct Sources<'a> {
    api: &'a Api,
    youtube: &'a Youtube,
    config: &'a SharedConfig,
}

/// A track's stream, resolved and ready to download.
struct Resolved {
    source: AudioSource,
    /// Seconds, from whichever API answered.
    duration_secs: u64,
    plan: DownloadPlan,
    /// The stream's total size, when the API states it up front (YouTube does).
    total: Option<u64>,
}

/// How a job's bytes are fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadPlan {
    /// One GET returns the whole stream (Bilibili).
    Single,
    /// Sequential ranged GETs.
    ///
    /// A plain GET of a `googlevideo` URL is throttled to a crawl — measured at
    /// ~0.03 MB/s, where 3.15 MB took 97 s — while ranged requests come back at
    /// full speed. That is the whole reason this exists.
    Ranges { first: u64, chunk: u64 },
}

/// The slice size for a ranged source. Large enough that the request overhead
/// disappears, small enough that a cancelled download has not wasted much.
const RANGE_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

/// How much media data to pull along with the container header.
///
/// The decoder needs `ftyp` + `moov` + `sidx` before it can be handed a single
/// sample, and playback should start as soon as it has them — so the first request
/// covers the header plus this much audio.
const RANGE_OPENING_BYTES: u64 = 512 * 1024;

impl DownloadPlan {
    /// The plan for a source that must be fetched in ranges.
    pub(crate) fn ranged(init_end: Option<u64>, index_end: Option<u64>) -> Self {
        // `indexRange.end` already includes the `sidx`; without it, the init
        // segment's end is the best available bound.
        let header = index_end.or(init_end).map(|end| end + 1).unwrap_or(0);
        DownloadPlan::Ranges {
            first: header + RANGE_OPENING_BYTES,
            chunk: RANGE_CHUNK_BYTES,
        }
    }

    /// The byte ranges to request, in order.
    ///
    /// A range is inclusive at both ends. The list covers every byte of the stream
    /// exactly once: a gap would corrupt the file, an overlap would duplicate
    /// samples and put the decoder out of step.
    pub(crate) fn ranges(self, total: u64) -> Vec<(u64, u64)> {
        if total == 0 {
            return Vec::new();
        }
        match self {
            DownloadPlan::Single => vec![(0, total - 1)],
            DownloadPlan::Ranges { first, chunk } => {
                let chunk = chunk.max(1);
                let mut ranges = Vec::new();
                let mut start = 0;
                while start < total {
                    let want = if start == 0 { first.max(1) } else { chunk };
                    let end = (start + want - 1).min(total - 1);
                    ranges.push((start, end));
                    start = end + 1;
                }
                ranges
            }
        }
    }
}

fn load_track(
    sources: &Sources<'_>,
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

    let resolved = match track.source {
        Source::Bilibili => resolve_bilibili(sources, &mut track, evt_tx),
        Source::Youtube => resolve_youtube(sources.youtube, &mut track, evt_tx),
    };
    let Some(resolved) = resolved else {
        // The resolver has already said what went wrong.
        return;
    };

    let key = track.key();
    if play {
        // Cache-only jobs must not disturb what the list shows as playing.
        let _ = evt_tx.send(Evt::TrackResolved(Box::new(track.clone())));
    }

    // Newest request wins; lets the download worker abandon older segments.
    // A generation counter rather than the cid, so re-clicking the same song
    // also supersedes its earlier download. Cache-only jobs stay out of that
    // race in both directions.
    let generation = play.then(|| request_id.fetch_add(1, Ordering::SeqCst) + 1);

    if let Some(path) = cache.get(&track.cache_key(), resolved.source.quality.stream_id()) {
        if play {
            // Fully cached: plain, non-blocking playback.
            let _ = evt_tx.send(Evt::TrackReady {
                track: Box::new(track.clone()),
                source: PlaybackSource::Complete(path),
                quality: resolved.source.quality,
            });
        } else if purpose == Purpose::Export {
            spawn_export(
                cache.clone(),
                track.clone(),
                resolved.source.quality,
                evt_tx.clone(),
            );
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
            source: resolved.source,
            plan: resolved.plan,
            total: resolved.total,
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
    // `duration_secs` is already folded into the track by the resolver.
    let _ = resolved.duration_secs;
}

/// Resolve a Bilibili track: its `cid`, then an audio URL, then a quality.
fn resolve_bilibili(
    sources: &Sources<'_>,
    track: &mut Track,
    evt_tx: &Sender<Evt>,
) -> Option<Resolved> {
    let api = sources.api;
    let config = sources.config;
    if let Err(err) = video::resolve_track(api, track) {
        send_error(evt_tx, "获取视频信息失败", err);
        return None;
    }
    if track.cid == 0 {
        let _ = evt_tx.send(Evt::Error {
            context: "播放失败".into(),
            message: "该视频没有可播放的音频流".into(),
        });
        return None;
    }

    let prefer_flac = config.lock().unwrap().prefer_flac;
    let playurl = match video::playurl(api, &track.bvid, track.cid) {
        Ok(data) => data,
        Err(err) => {
            send_error(evt_tx, "获取音频地址失败", err);
            return None;
        }
    };

    let Some(source) = video::pick_audio(&playurl, prefer_flac) else {
        let _ = evt_tx.send(Evt::Error {
            context: "播放失败".into(),
            message: format!("没有可用的音频流（{}）", video::describe_streams(&playurl)),
        });
        return None;
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

    Some(Resolved {
        source,
        duration_secs,
        // Bilibili serves one file in one response.
        plan: DownloadPlan::Single,
        total: None,
    })
}

/// Resolve a YouTube track: the player's own metadata, and a decodable stream.
fn resolve_youtube(youtube: &Youtube, track: &mut Track, evt_tx: &Sender<Evt>) -> Option<Resolved> {
    let playable = match youtube.resolve(&track.bvid) {
        Ok(playable) => playable,
        Err(err) => {
            let _ = evt_tx.send(Evt::Error {
                context: format!("无法播放：{}", track.title),
                message: err.to_string(),
            });
            return None;
        }
    };

    if playable.is_live {
        let _ = evt_tx.send(Evt::Error {
            context: "无法播放".into(),
            message: "直播流没有固定长度，无法缓存与拖动进度".into(),
        });
        return None;
    }

    // Fill in what a search row could not know, so the player bar and the cache
    // export are named properly.
    if !playable.title.is_empty() {
        track.title = playable.title.clone();
    }
    if !playable.author.is_empty() {
        track.author = playable.author.clone();
    }
    if playable.duration > 0 {
        track.duration = playable.duration;
    }

    let Some(pick) = playable.best_audio().cloned() else {
        let _ = evt_tx.send(Evt::Error {
            context: "无法播放".into(),
            message: "该视频没有可解码的音轨（YouTube 未提供 AAC，只有 Opus/WebM）".into(),
        });
        return None;
    };

    Some(Resolved {
        source: AudioSource {
            quality: pick.quality,
            urls: vec![pick.url],
        },
        duration_secs: playable.duration,
        plan: DownloadPlan::ranged(pick.init_end, pick.index_end),
        total: pick.content_length,
    })
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
    youtube: Arc<Youtube>,
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
        let lyrics = lyrics::fetch_for(&api, &youtube, &track, &root);
        // A stale answer is harmless: the UI matches on the track key.
        let _ = evt_tx.send(Evt::LyricsReady { key, lyrics });
    }
}

// ---------------------------------------------------------------------------
// Download worker
// ---------------------------------------------------------------------------

/// A fresh stream URL for a track whose signed one stopped working.
///
/// Only the ranged sources need this: their URL is signed and expires, while a
/// Bilibili CDN URL is re-fetched by the caller's own retry loop.
fn refresh_stream_url(sources: &Sources<'_>, track: &Track) -> Option<String> {
    let Source::Youtube = track.source else {
        return None;
    };
    match sources.youtube.resolve(&track.bvid) {
        Ok(playable) => playable.best_audio().map(|pick| pick.url.clone()),
        Err(err) => {
            eprintln!("re-resolving {} failed: {err}", track.bvid);
            None
        }
    }
}

/// Remux a cached segment into a normal audio file on its own thread.
///
/// The copy is hundreds of megabytes and must not stall the worker that serves
/// playback, so it gets a thread rather than a slot in the queue.
fn spawn_export(cache: Arc<AudioCache>, track: Track, quality: AudioQuality, evt_tx: Sender<Evt>) {
    let title = track.title.clone();
    let _ = std::thread::Builder::new()
        .name("listenbli-export".into())
        .spawn(move || {
            let Some(segment) = cache.get(&track.cache_key(), quality.stream_id()) else {
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
    youtube: Arc<Youtube>,
    config: SharedConfig,
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
        // Built per job: `Sources` borrows, and the borrows must not outlive the
        // loop body that uses them.
        let sources = Sources {
            api: &api,
            youtube: &youtube,
            config: &config,
        };
        if let Err(message) = run_download(&sources, &job, &evt_tx, &request_id, &cache) {
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
    sources: &Sources<'_>,
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
        match stream_segment(sources, url, job, evt_tx, request_id, cache) {
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
    sources: &Sources<'_>,
    url: &str,
    job: &DownloadJob,
    evt_tx: &Sender<Evt>,
    request_id: &AtomicU64,
    cache: &Arc<AudioCache>,
) -> Result<(), String> {
    let api = sources.api;
    let key = job.track.key();
    let cache_key = job.track.cache_key();
    let quality = job.source.quality;
    let tag = quality.stream_id();

    // A single GET answers with the whole stream and its length. A ranged source
    // states the length up front instead — and it *must*, because the
    // `Content-Length` of a 206 response is only the length of that chunk.
    let mut single = match job.plan {
        DownloadPlan::Single => Some(
            api.get_stream(url, Some(BILI_WEB))
                .map_err(|e| e.to_string())?,
        ),
        DownloadPlan::Ranges { .. } => None,
    };
    let total = match single.as_ref() {
        Some(response) => response
            .content_length()
            .ok_or_else(|| "CDN 未返回 Content-Length，无法流式播放".to_string())?,
        None => job
            .total
            .ok_or_else(|| "该接口未给出音频长度，无法按范围下载".to_string())?,
    };
    if total == 0 {
        return Err("CDN 返回了空内容".to_string());
    }

    platform::ensure_dir(&cache.dir()).map_err(|e| format!("创建缓存目录失败: {e}"))?;
    let path = cache.path_for(&cache_key, tag);

    // Fresh attempt: drop any stale partial data and record what to expect.
    // The sidecar is written first, so a `.m4s` without one is never a hit.
    cache.remove(&cache_key, tag);
    cache
        .begin(&cache_key, tag, total)
        .map_err(|e| format!("写入缓存元数据失败: {e}"))?;
    let mut file = std::fs::File::create(&path).map_err(|e| format!("创建缓存文件失败: {e}"))?;

    let state = Arc::new(StreamState::new(total));

    // Any failure must also fail the stream state, otherwise a decoder already
    // reading this file would block until the read timeout.
    let abort = |message: String| -> String {
        state.fail(message.clone());
        cache.remove(&cache_key, tag);
        message
    };

    let mut chunk = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    let mut announced = false;
    let mut last_reported = 0u64;

    // Both paths copy bytes the same way, so the announce/progress/publish rules
    // live in one place. The closure borrows the counters it advances, which is
    // why it is scoped: the completeness check below needs them back.
    let mut copy_into_cache = |response: &mut reqwest::blocking::Response| -> Result<(), String> {
        loop {
            if is_superseded(request_id, job.generation) {
                state.cancel();
                cache.remove(&cache_key, tag);
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
        Ok(())
    };

    match single.as_mut() {
        Some(response) => copy_into_cache(response)?,
        None => {
            // A signed stream URL is short-lived, and a long download can outlive
            // it. One refresh is allowed, and the retry asks for the same range:
            // that request never delivered a byte, so the range is exactly where
            // the file stopped — the chunks already written are not fetched again.
            // A failure *inside* a body is different: it is not retried here,
            // because resuming mid-range would need the byte counter that the copy
            // loop owns; the caller's next URL restarts the segment instead.
            let mut url = url.to_owned();
            let mut refreshed = false;
            for (start, end) in job.plan.ranges(total) {
                let want = end - start + 1;
                let range = format!("bytes={start}-{end}");
                let mut response = match api.get_stream_range(&url, &range) {
                    Ok(response) => response,
                    Err(err) => {
                        if refreshed {
                            return Err(abort(format!("{range} 请求失败（已重试过）：{err}")));
                        }
                        refreshed = true;
                        let Some(fresh) = refresh_stream_url(sources, &job.track) else {
                            return Err(abort(format!(
                                "{range} 请求失败，且无法重新取得地址：{err}"
                            )));
                        };
                        eprintln!("stream URL expired mid-download; refreshed it once");
                        url = fresh;
                        api.get_stream_range(&url, &range)
                            .map_err(|err| abort(format!("{range} 重新请求仍失败：{err}")))?
                    }
                };
                // A source that ignored the range would answer 200 with the whole
                // file; writing that per chunk would corrupt the cache, so the
                // length is checked rather than assumed.
                if response.content_length() != Some(want) {
                    return Err(abort(format!(
                        "该地址忽略了范围请求（{range}：想要 {want} 字节）"
                    )));
                }
                copy_into_cache(&mut response)?;
            }
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
    if let Err(message) = cache.finish_download(&cache_key, tag) {
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

    /// A missing byte would corrupt the file, a repeated one would feed the
    /// decoder a sample twice, so the range list has to tile the stream exactly.
    fn covers(ranges: &[(u64, u64)], total: u64) {
        assert!(
            !ranges.is_empty(),
            "a non-empty stream needs at least one range"
        );
        assert_eq!(ranges[0].0, 0, "the first range must start at zero");
        let mut at = 0u64;
        for (index, (start, end)) in ranges.iter().enumerate() {
            assert!(*end >= *start, "range {index} is inverted: {start}-{end}");
            assert_eq!(
                *start, at,
                "range {index} starts at {start} but the previous ended at {at}"
            );
            at = end + 1;
        }
        assert_eq!(at, total, "the ranges must end exactly at the stream's end");
    }

    #[test]
    fn a_ranged_plan_tiles_the_stream_exactly() {
        let plan = DownloadPlan::ranged(Some(722), Some(1018));
        let total = 3_449_447;

        let ranges = plan.ranges(total);
        covers(&ranges, total);

        // The first range covers the container header plus the opening audio, so
        // the decoder can start on the very first response.
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges[0].1, 1018 + RANGE_OPENING_BYTES);
        // This stream is only three chunks long, so the second range is also the
        // last: what a *long* stream looks like is checked separately.
        assert_eq!(ranges.len(), 2);

        let long = plan.ranges(100 * 1024 * 1024);
        covers(&long, 100 * 1024 * 1024);
        assert_eq!(
            long[1].1 - long[1].0 + 1,
            RANGE_CHUNK_BYTES,
            "every range after the opening one is a full chunk"
        );
    }

    /// The last range has to stop at the end of the stream rather than at the
    /// chunk boundary, or the download would overshoot and the completeness check
    /// would fail.
    #[test]
    fn the_last_range_is_clamped_to_the_stream() {
        let plan = DownloadPlan::Ranges {
            first: 10,
            chunk: 100,
        };
        let ranges = plan.ranges(250);
        covers(&ranges, 250);
        assert_eq!(*ranges.last().unwrap(), (210, 249));
    }

    /// A range larger than the file, which is what a short video looks like, must
    /// produce exactly one range rather than an empty or inverted one.
    #[test]
    fn a_stream_smaller_than_the_first_range_is_one_range() {
        let plan = DownloadPlan::ranged(None, None);
        let ranges = plan.ranges(1024);
        covers(&ranges, 1024);
        assert_eq!(ranges, vec![(0, 1023)]);
    }

    /// Without the response's ranges the opening chunk is still a sensible request,
    /// and it must never be zero-length.
    #[test]
    fn a_plan_without_byte_ranges_still_opens_usefully() {
        let plan = DownloadPlan::ranged(None, None);
        match plan {
            DownloadPlan::Ranges { first, chunk } => {
                assert_eq!(first, RANGE_OPENING_BYTES);
                assert!(chunk > 0);
            }
            other => panic!("expected ranges, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_stream_asks_for_nothing() {
        assert!(DownloadPlan::Single.ranges(0).is_empty());
        assert!(DownloadPlan::ranged(None, None).ranges(0).is_empty());
    }

    /// The single-GET source is one range covering everything, which is how the
    /// Bilibili path behaves through the same code.
    #[test]
    fn a_single_get_is_one_whole_range() {
        assert_eq!(DownloadPlan::Single.ranges(2048), vec![(0, 2047)]);
    }

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

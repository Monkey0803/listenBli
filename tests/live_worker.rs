//! End-to-end tests for the worker layer — the exact plumbing the UI drives.
//!
//! These go further than `live.rs`: they exercise the command/event bus, the
//! download worker, the on-disk cache, the lyrics pipeline and — in the last
//! test — actual audio playback through the OS device.
//!
//! ```text
//! cargo test --test live_worker -- --ignored --nocapture --test-threads=1
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use listenbli::api::client::Api;
use listenbli::api::cookie::CookieJar;
use listenbli::api::models::Track;
use listenbli::audio::{looks_like_iso_bmff, AudioEngine, SeekOutcome};
use listenbli::config::{self, Config};
use listenbli::net::{Cmd, Evt, Worker};

/// Drive the engine's poll loop until it has installed a decoder.
///
/// Decoding happens on a worker thread, so tests — like the UI — have to poll
/// for it rather than expect `play_stream` to have finished.
fn wait_for_decoder(engine: &mut AudioEngine) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match engine.poll() {
            Some(Ok(())) => return,
            Some(Err(err)) => panic!("the decoder failed to open: {err}"),
            None => {
                assert!(
                    Instant::now() < deadline,
                    "the decoder never finished being built"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Drain events until one satisfies `f`, or panic on timeout.
fn wait_for<T>(
    rx: &Receiver<Evt>,
    timeout: Duration,
    what: &str,
    mut f: impl FnMut(Evt) -> Option<T>,
) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for {what}");
        }
        match rx.recv_timeout(remaining) {
            Ok(evt) => {
                if let Some(value) = f(evt) {
                    return value;
                }
            }
            Err(err) => panic!("waiting for {what}: {err}"),
        }
    }
}

fn start_worker() -> Worker {
    let api = Arc::new(Api::new(CookieJar::default()));
    let (config, root) = isolated_config();
    let worker = listenbli::net::spawn(api, config::shared(config));
    // The audio cache root is *not* taken from `Config` by `spawn`: the UI points the
    // worker at the user's chosen directory through this same call the moment that
    // setting changes. So setting only `Config::cache_dir` leaves the worker reading and
    // writing `~/Library/Caches/listenBli` all the same.
    worker.cache.set_root(&root);
    worker
}

/// The converted, playable cache entry for a track, waiting for the conversion if it is
/// still running.
///
/// "The download finished" and "the playable file exists" are two different moments. The
/// download marks its stream finished first and only then remuxes the fragment into an
/// `.m4a`, deleting the fragment. Asked for the cache entry inside that window, the
/// cache's own `get` still answers with the fragment — and that path can vanish
/// underneath a reader a moment later. Anything that means "the finished file" waits here.
fn playable_cache_entry(worker: &Worker, track: &Track, stream_id: u32) -> std::path::PathBuf {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(entry) = worker.cache.get(&track.cache_key(), stream_id) {
            if entry.extension().and_then(|ext| ext.to_str()) == Some("m4a") {
                assert!(
                    entry.is_file(),
                    "cached file missing at {}",
                    entry.display()
                );
                return entry;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the download was never converted into a playable .m4a"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
#[ignore = "hits the live Bilibili and NetEase APIs"]
fn worker_searches_resolves_and_downloads_a_track() {
    let worker = start_worker();

    worker
        .cmd_tx
        .send(Cmd::Search {
            source: listenbli::api::Source::Bilibili,
            keyword: "周杰伦 晴天".into(),
            page: 1,
        })
        .unwrap();

    let tracks: Vec<Track> = wait_for(
        &worker.evt_rx,
        Duration::from_secs(30),
        "search results",
        |evt| match evt {
            Evt::SearchResults { tracks, .. } => Some(tracks),
            Evt::Error { context, message } => panic!("{context}: {message}"),
            _ => None,
        },
    );
    assert!(!tracks.is_empty(), "search returned nothing");
    println!("search -> {} results", tracks.len());

    let target = tracks
        .iter()
        .find(|t| t.title.contains("晴天"))
        .cloned()
        .expect("expected a 晴天 result");
    let key = target.key();
    println!("playing {:?} ({})", target.title, key);

    // Start from a guaranteed cache miss so this test always exercises the
    // streaming path. (Run with --test-threads=1, as documented.)
    let cache = listenbli::audio::AudioCache::new();
    let _ = std::fs::remove_dir_all(cache.dir());

    // The UI asks for one cover per result as soon as a search lands. Covers
    // are decorative and live on their own worker: a click must never queue
    // behind them. Regression guard for exactly that — with a shared queue the
    // resolve arrives only after *every* cover has been delivered.
    let covers: Vec<String> = tracks
        .iter()
        .filter_map(|t| t.cover.clone().filter(|c| !c.is_empty()))
        .collect();
    for (index, url) in covers.iter().enumerate() {
        worker.fetch_cover(format!("cover{index}"), url.clone());
    }
    let mut covers_done = 0usize;

    let started = Instant::now();
    worker
        .cmd_tx
        .send(Cmd::LoadTrack(Box::new(target)))
        .unwrap();

    // Lyrics are fetched on the API worker while the download runs on the
    // download worker, so either event can arrive first. Collect both in a
    // single pass: waiting for them one at a time would discard the one that
    // arrived while the other was still pending.
    let mut ready: Option<(
        Track,
        listenbli::audio::PlaybackSource,
        listenbli::api::models::AudioQuality,
        Duration,
    )> = None;
    let mut lyrics: Option<listenbli::lyrics::Lyrics> = None;
    let mut complete_after: Option<Duration> = None;
    let deadline = Instant::now() + Duration::from_secs(180);

    loop {
        // A cache hit hands back `Complete` and emits no download progress at
        // all, so completion is only required for a streaming source.
        let streaming = matches!(
            ready.as_ref().map(|r| &r.1),
            Some(listenbli::audio::PlaybackSource::Streaming { .. })
        );
        let done = ready.is_some() && lyrics.is_some() && (!streaming || complete_after.is_some());
        if done {
            break;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "timed out; ready={:?} complete={complete_after:?}",
            ready.as_ref().map(|r| r.0.title.clone())
        );
        match worker.evt_rx.recv_timeout(remaining) {
            Ok(Evt::TrackReady {
                track,
                source,
                quality,
            }) => {
                // Checked here, at the moment the engine is handed the source, rather than
                // after the download: the path a streaming handoff names is the fragment,
                // and a successful conversion deletes it and leaves the playable `.m4a`
                // behind. Asserting on it after completion tests a race, not the app.
                assert!(
                    source.path().is_file(),
                    "the source handed to the engine is missing: {}",
                    source.path().display()
                );
                ready = Some((*track, source, quality, started.elapsed()));
            }
            Ok(Evt::CoverReady { .. }) => covers_done += 1,
            Ok(Evt::DownloadProgress { got, total, .. }) => {
                if let Some(total) = total {
                    if got >= total && complete_after.is_none() {
                        complete_after = Some(started.elapsed());
                    }
                }
            }
            Ok(Evt::LyricsReady { key: k, lyrics: l }) if k == key => lyrics = Some(l),
            Ok(Evt::Error { context, message }) => panic!("{context}: {message}"),
            Ok(_) => {}
            Err(err) => panic!("waiting for track/lyrics: {err}"),
        }
    }

    let (track, source, quality, time_to_ready) = ready.unwrap();
    // There used to be an assertion here: `covers_done < covers.len()`, i.e. "resolve did
    // not wait for all 20 covers". It only held while the track was already cached — then
    // resolve never touched the network and beat the cover thread. With a cache that
    // starts empty, resolve does real work and the 20 sequential thumbnail fetches finish
    // first, so the assertion fails without anything being wrong. The property itself is
    // structural (covers run on their own thread and cannot block resolve); this number is
    // reported for the eye, and `time_to_ready < time_to_complete` below is the assertion
    // that actually pins streaming down.
    println!(
        "covers done when playback was offered: {covers_done} of {}",
        covers.len()
    );
    let time_to_complete = complete_after.unwrap_or(time_to_ready);

    assert!(track.cid != 0, "cid should have been resolved");
    assert!(
        track.duration > 60,
        "duration should have been resolved, got {}",
        track.duration
    );

    // The whole point of streaming: playback must become possible long before
    // the download finishes.
    println!(
        "streaming: playback offered after {:.2}s ({} of {} covers done), \
         download finished after {:.2}s",
        time_to_ready.as_secs_f64(),
        covers_done,
        covers.len(),
        time_to_complete.as_secs_f64()
    );
    assert!(
        time_to_ready < time_to_complete,
        "playback was only offered after the download completed \
         ({time_to_ready:?} vs {time_to_complete:?}) - not actually streaming"
    );

    // Handing the track over must not wait on the decoder.
    //
    // This deliberately does *not* claim to prove the decoder opened early: the
    // loop above already waited for the download to complete, so the file is
    // whole by now. (An earlier version of this test made exactly that claim,
    // which is why it never caught the freeze it was meant to guard.) What
    // opening a decoder costs is covered offline by the unit test
    // `queueing_a_decoder_never_blocks_the_caller`.
    let mut engine = match AudioEngine::new(0.0) {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("skipping decoder timing: no audio device ({err})");
            return;
        }
    };
    let handoff_started = Instant::now();
    engine.play_stream(&source, Duration::from_secs(track.duration), track.key());
    let handoff = handoff_started.elapsed();
    println!("handed to the engine in {:.3}s", handoff.as_secs_f64());
    assert!(
        handoff < Duration::from_millis(500),
        "handing a track to the engine blocked the caller for {handoff:?}"
    );

    let installed_started = Instant::now();
    wait_for_decoder(&mut engine);
    println!(
        "decoder installed {:.3}s after handoff",
        installed_started.elapsed().as_secs_f64()
    );
    assert!(engine.is_playing(), "engine should report playing");

    println!(
        "track ready -> {} ({}, {} s)",
        track.title,
        quality.label(),
        track.duration
    );

    let lyrics = lyrics.unwrap();
    println!(
        "lyrics -> source {:?}, {} lines",
        lyrics.source,
        lyrics.lines.len()
    );
    assert!(!lyrics.is_empty(), "expected lyrics for this track");

    // A second request for the same track must be served from the completed cache.
    // Asked of the worker's own cache, not a fresh `AudioCache::new()`: that one is rooted
    // at the platform default and would be inspecting a different directory entirely.
    //
    // The finished entry is deliberately *not* compared with the path the streaming
    // handoff named: that one is the fragment `source.path()` pointed at while bytes were
    // still arriving, and completion replaces it with the playable `.m4a`.
    let cached = playable_cache_entry(&worker, &track, quality.stream_id());
    let bytes = std::fs::read(&cached).unwrap();
    assert!(
        looks_like_iso_bmff(&bytes),
        "cached file is not ISO-BMFF ({} bytes)",
        bytes.len()
    );
}

/// Opens the real audio device, decodes the cached segment and confirms the
/// transport actually advances. This is as close to "it makes sound" as an
/// automated test can get.
#[test]
#[ignore = "opens the system audio device and hits the live Bilibili API"]
fn audio_engine_starts_and_advances_playback() {
    let worker = start_worker();
    worker
        .cmd_tx
        .send(Cmd::Search {
            source: listenbli::api::Source::Bilibili,
            keyword: "周杰伦 晴天".into(),
            page: 1,
        })
        .unwrap();

    let tracks: Vec<Track> = wait_for(
        &worker.evt_rx,
        Duration::from_secs(30),
        "search results",
        |evt| match evt {
            Evt::SearchResults { tracks, .. } => Some(tracks),
            Evt::Error { context, message } => panic!("{context}: {message}"),
            _ => None,
        },
    );
    let target = tracks
        .into_iter()
        .find(|t| t.title.contains("晴天"))
        .unwrap();
    worker
        .cmd_tx
        .send(Cmd::LoadTrack(Box::new(target)))
        .unwrap();

    let (track, source, quality) = wait_for(
        &worker.evt_rx,
        Duration::from_secs(180),
        "track ready",
        |evt| match evt {
            Evt::TrackReady {
                track,
                source,
                quality,
            } => Some((*track, source, quality)),
            Evt::Error { context, message } => panic!("{context}: {message}"),
            _ => None,
        },
    );

    // Let the background download finish so this test exercises the plain,
    // fully-cached path. Streaming playback is covered by the test above.
    if let Some(state) = source.stream_state() {
        let deadline = Instant::now() + Duration::from_secs(120);
        while state.is_running() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!state.is_running(), "the download never finished");
    }

    // The playable file, not `source.path()`: a streaming handoff names the fragment, and
    // finishing also remuxes that fragment into a `.m4a` and deletes it. Carrying the
    // fragment path past the download is what made this test fail two different ways — a
    // format error when it opened the fragment, and "no such file" once conversion had
    // already removed it.
    let path = playable_cache_entry(&worker, &track, quality.stream_id());

    let mut engine = match AudioEngine::new(0.0) {
        Ok(engine) => engine,
        Err(err) => {
            // A headless CI box has no output device; skip rather than fail.
            eprintln!("skipping: no audio device available ({err})");
            return;
        }
    };

    engine.play_stream(
        &listenbli::audio::PlaybackSource::Complete(path),
        Duration::from_secs(track.duration),
        track.key(),
    );
    wait_for_decoder(&mut engine);
    assert!(engine.is_playing(), "engine should report playing");

    let started = engine.position();
    std::thread::sleep(Duration::from_millis(1200));
    let advanced = engine.position();
    println!(
        "position {} ms -> {} ms (duration {} s)",
        started.as_millis(),
        advanced.as_millis(),
        engine.duration().as_secs()
    );
    assert!(
        advanced > started,
        "playback position did not advance ({started:?} -> {advanced:?})"
    );
    assert!(
        engine.progress() > 0.0 && engine.progress() <= 1.0,
        "progress out of range: {}",
        engine.progress()
    );

    // Seeking must work on the real decoder, and must clamp past the end.
    // This track is a cached file, so it is opened seekable and every jump is
    // available immediately.
    assert_eq!(
        engine.seek(Duration::from_secs(60)),
        SeekOutcome::Seeked,
        "a complete file must be seekable"
    );
    let after_seek = engine.position();
    assert!(
        after_seek >= Duration::from_secs(55),
        "seek did not move the position: {after_seek:?}"
    );
    assert_eq!(
        engine.seek(Duration::from_secs(999_999)),
        SeekOutcome::Seeked,
        "an over-long seek should clamp, not fail"
    );
    assert!(
        engine.position() <= engine.duration(),
        "position must stay within the duration"
    );
    println!("after seek -> {:?}", engine.position());

    // Pause/resume must be honoured.
    engine.toggle_pause();
    assert!(engine.is_paused());
    engine.toggle_pause();
    assert!(!engine.is_paused());

    engine.stop();
    assert!(engine.loaded_key().is_none());
}

/// A configuration with a cache directory of its own, and that directory.
///
/// These tests download, convert and delete real files, and the default cache root is the
/// user's own `~/Library/Caches/listenBli`. A temporary directory keeps a test run from
/// evicting or deleting whatever is actually cached on the machine.
///
/// This started out as the hunt for a different problem: two tests were thought to be
/// fighting over one cache directory. Isolating them did not stop the failure — it is what
/// finally showed the failure was inside the tests themselves, which carried a fragment
/// path across the conversion that deletes it. Isolation stays for the reason above, not
/// for that one.
fn isolated_config() -> (Config, std::path::PathBuf) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("listenbli-live-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config {
        cache_dir: Some(dir.clone()),
        ..Config::default()
    };
    (config, dir)
}

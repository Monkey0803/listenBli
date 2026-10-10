//! Playback engine: a thin safe wrapper over rodio's `Player`.
//!
//! The M0 spike established two things this wrapper exists to absorb:
//! * `Decoder::total_duration()` is `None` for Bilibili's DASH fMP4 segments,
//!   so the duration always has to come from the API; and
//! * `Player::try_seek` cannot saturate at the end when the source does not
//!   report a duration, so positions must be clamped by us.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use rodio::decoder::DecoderBuilder;
use rodio::{Decoder, MixerDeviceSink, Player, Source};

use super::stream::{BlockingFileReader, PlaybackSource, StreamState, StreamStatus};

/// A decoder that has been built and is ready to hand to the player.
///
/// Boxed rather than a concrete `Decoder<R>` because building it happens on a
/// worker thread and what comes back has to cross a channel.
pub type PreparedSource = Box<dyn Source + Send>;

/// What a worker thread needs in order to open a decoder.
enum PrepareJob {
    Complete(PathBuf),
    Streaming {
        path: PathBuf,
        total: u64,
        state: Arc<StreamState>,
        /// Whether to declare the source seekable.
        ///
        /// This is the single most consequential flag in the file. Symphonia's
        /// ISO-BMFF reader only stops early — at the first `moof`/`mdat` — when
        /// the source is *un*seekable; a seekable source makes it scan every
        /// atom to build its index, which means reading to the end of the file.
        /// Measured on a real 181.9 MB segment:
        ///
        /// * `seekable(true)`: `build()` took 3.6 s, having waited for 100% of
        ///   the file to arrive, across 7983 reads;
        /// * `seekable(false)`: `build()` took 1.1 ms after **5 reads and 30 KB**.
        ///
        /// So a file still arriving must be opened unseekable to play early. The
        /// cost is that symphonia will then only move *forward* (by ignoring
        /// bytes) — see `AudioEngine::seek`, which re-opens the file seekably
        /// once it is complete so that jumping back still works.
        seekable: bool,
    },
}

impl PrepareJob {
    /// A job for a track that is still arriving.
    fn streaming(source: &PlaybackSource) -> Option<Self> {
        match source {
            PlaybackSource::Streaming { path, total, state } => Some(PrepareJob::Streaming {
                path: path.clone(),
                total: *total,
                state: Arc::clone(state),
                // Play as soon as the first fragment lands; seeking is handled
                // by `AudioEngine::seek` re-opening once the file is complete.
                seekable: false,
            }),
            PlaybackSource::Complete(_) => None,
        }
    }
}

/// Open a decoder. **Blocking, and expensive — never call this on the UI thread.**
///
/// With `seekable` set this does not return until it has read the file to the
/// end, so for a segment that is still arriving it is bounded only by the
/// download; see [`PrepareJob::Streaming::seekable`] for the measurements. This
/// always runs on a worker thread, which is why it may block freely.
fn build_source(job: PrepareJob) -> Result<PreparedSource, String> {
    match job {
        PrepareJob::Complete(path) => {
            let file = std::fs::File::open(&path)
                .map_err(|e| format!("无法打开音频缓存 {}: {e}", path.display()))?;
            let decoder = Decoder::try_from(file)
                .map_err(|e| format!("解码失败（音频格式可能不受支持）: {e}"))?;
            Ok(Box::new(decoder))
        }
        PrepareJob::Streaming {
            path,
            total,
            state,
            seekable,
        } => {
            let reader = BlockingFileReader::open(&path, state)
                .map_err(|e| format!("无法打开音频缓存 {}: {e}", path.display()))?;
            let decoder = DecoderBuilder::new()
                .with_data(reader)
                .with_byte_len(total)
                .with_seekable(seekable)
                .build()
                .map_err(|e| format!("解码失败（音频格式可能不受支持）: {e}"))?;
            Ok(Box::new(decoder))
        }
    }
}

/// Queue a decoder build and return its result channel at once.
///
/// One thread per request on purpose: a superseded job can be parked in a
/// blocking read for up to `stream::READ_TIMEOUT`, and the newest click must not
/// queue behind it. Abandoned jobs end early anyway, because the download worker
/// cancels their `StreamState` when a newer track supersedes them.
/// The file to re-open when a jump needs a seekable reader.
///
/// Usually the path we were handed, but the cache replaces a finished fragment
/// with a progressive `.m4a` and drops the fragment, and a track that is playing
/// right now keeps reading through its already-open handle. So the file named by
/// `path` can be gone while the music plays on, and the copy beside it holds the
/// same audio.
fn seekable_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path.to_path_buf();
    }
    let sibling = path.with_extension("m4a");
    if sibling.exists() {
        sibling
    } else {
        path.to_path_buf()
    }
}

fn spawn_prepare(job: PrepareJob) -> Receiver<Result<PreparedSource, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // A closed receiver just means the engine moved on; ignore it.
        let _ = tx.send(build_source(job));
    });
    rx
}

/// A decoder being built on a worker thread.
struct Pending {
    rx: Receiver<Result<PreparedSource, String>>,
    duration: Duration,
    key: String,
    stream: Option<Arc<StreamState>>,
    path: PathBuf,
}

/// A seekable decoder being opened over a file that has finished downloading,
/// so that a jump backwards becomes possible again.
struct Upgrade {
    rx: Receiver<Result<PreparedSource, String>>,
    target: Duration,
}

/// What a seek request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekOutcome {
    /// The source moved.
    Seeked,
    /// The jump needs a seekable view of the file, which is being opened; it
    /// will happen when that lands.
    Upgrading,
    /// Nothing is loaded, or the wanted position is behind the play head while
    /// the file is still arriving.
    Refused,
}

/// Clamp a requested position into `0..=duration`.
pub fn clamp_position(position: Duration, duration: Duration) -> Duration {
    if position > duration {
        duration
    } else {
        position
    }
}

/// Fraction played, `0.0..=1.0`. A zero duration yields 0.
pub fn progress_ratio(position: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 0.0;
    }
    let ratio = position.as_secs_f64() / duration.as_secs_f64();
    ratio.clamp(0.0, 1.0) as f32
}

pub struct AudioEngine {
    /// Dropping the device sink stops all sound, so it must be kept alive.
    _sink: MixerDeviceSink,
    player: Player,
    duration: Duration,
    volume: f32,
    /// `None` when nothing has been loaded (which is how we distinguish
    /// "stopped" from "finished playing").
    loaded_key: Option<String>,
    /// Set while the current track is still being streamed in.
    stream: Option<Arc<StreamState>>,
    /// A decoder still being built for the newest selection.
    pending: Option<Pending>,
    /// Whether the *installed* decoder can seek freely. False while a segment is
    /// being played unseekable so that it could start early.
    source_seekable: bool,
    /// The file behind the installed decoder, needed to re-open it seekably.
    path: PathBuf,
    /// A seekable re-open in flight, requested by a jump the current source
    /// cannot make.
    upgrade: Option<Upgrade>,
}

impl AudioEngine {
    pub fn new(volume: f32) -> Result<Self, String> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("无法打开音频输出设备: {e}"))?;
        let player = Player::connect_new(sink.mixer());
        player.set_volume(volume);
        Ok(Self {
            _sink: sink,
            player,
            duration: Duration::ZERO,
            volume,
            loaded_key: None,
            stream: None,
            pending: None,
            source_seekable: true,
            path: PathBuf::new(),
            upgrade: None,
        })
    }

    /// Hand a track to the player, returning immediately.
    ///
    /// Decoding is queued to a worker thread and lands via [`AudioEngine::poll`].
    /// Nothing here blocks: the window must stay responsive while a segment is
    /// being read, which for a long upload is the whole download.
    ///
    /// A segment still arriving is opened *unseekable* so that playback can begin
    /// as soon as its first fragment lands rather than after the whole file
    /// (see [`PrepareJob::Streaming::seekable`]). A file already on disk is
    /// opened seekable, as before.
    ///
    /// Playback of the previous track stops now, because the new one cannot
    /// start instantly and letting the old song continue under a new title would
    /// misrepresent what is happening.
    pub fn play_stream(
        &mut self,
        source: &PlaybackSource,
        duration: Duration,
        key: impl Into<String>,
    ) {
        let job = match PrepareJob::streaming(source) {
            Some(job) => job,
            None => PrepareJob::Complete(source.path().to_path_buf()),
        };
        self.pending = Some(Pending {
            rx: spawn_prepare(job),
            duration,
            key: key.into(),
            stream: source.stream_state().cloned(),
            path: source.path().to_path_buf(),
        });
        self.upgrade = None;
        self.player.stop();
        self.loaded_key = None;
        self.stream = None;
        self.duration = duration;
    }

    /// Install a decoder that finished building, or a seekable re-open that did.
    ///
    /// `None` while nothing is ready; `Some(Ok(()))` once something was
    /// installed; `Some(Err(_))` if it could not be opened.
    pub fn poll(&mut self) -> Option<Result<(), String>> {
        if let Some(outcome) = self.poll_upgrade() {
            return Some(outcome);
        }

        let pending = self.pending.as_ref()?;
        match pending.rx.try_recv() {
            Ok(Ok(source)) => {
                let Pending {
                    duration,
                    key,
                    stream,
                    path,
                    ..
                } = self.pending.take().expect("checked just above");
                let seekable = stream.is_none();
                self.path = path;
                self.install(source, duration, key, stream, seekable, None);
                Some(Ok(()))
            }
            Ok(Err(err)) => {
                self.pending = None;
                Some(Err(err))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                Some(Err("解码线程意外退出".to_string()))
            }
        }
    }

    /// Install a seekable re-open once it lands, jumping to the position that
    /// asked for it.
    ///
    /// Reports like [`AudioEngine::poll`] does, so a caller that is waiting for a
    /// jump to happen can see it: the audio keeps playing throughout, so there is
    /// no other signal that the swap occurred.
    /// The path to hand to a seekable re-open, resolved when the jump is made
    /// rather than when the source was installed.
    fn seekable_source(&self) -> PathBuf {
        seekable_path(&self.path)
    }

    fn poll_upgrade(&mut self) -> Option<Result<(), String>> {
        let upgrade = self.upgrade.as_ref()?;
        let arrived = match upgrade.rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("解码线程意外退出".to_string()),
        };
        let target = self.upgrade.take().expect("checked just above").target;
        match arrived {
            Ok(source) => {
                let duration = self.duration;
                let key = self.loaded_key.clone().unwrap_or_default();
                // The file is whole now, so `stream` clears: reads no longer block.
                self.install(source, duration, key, None, true, Some(target));
                Some(Ok(()))
            }
            // The old source is still playing; only the jump was lost.
            Err(err) => Some(Err(err)),
        }
    }

    /// True while the newest selection's decoder is still being built.
    pub fn is_preparing(&self) -> bool {
        self.pending.is_some()
    }

    fn install(
        &mut self,
        source: PreparedSource,
        duration: Duration,
        key: String,
        stream: Option<Arc<StreamState>>,
        seekable: bool,
        resume_at: Option<Duration>,
    ) {
        self.player.stop();
        self.duration = duration;
        self.stream = stream;
        self.source_seekable = seekable;
        self.player.append(source);
        self.player.set_volume(self.volume);
        self.player.play();
        self.loaded_key = Some(key);
        if let Some(target) = resume_at {
            let _ = self.player.try_seek(clamp_position(target, duration));
        }
    }

    /// The in-flight stream for the current track, if any.
    pub fn stream(&self) -> Option<&Arc<StreamState>> {
        self.stream.as_ref()
    }

    pub fn stop(&mut self) {
        // Just drop the reference: the source itself is discarded by
        // `player.stop()`, and deliberately *not* cancelling the in-flight
        // download means it still finishes and leaves a complete cache entry for
        // the next play.
        self.pending = None;
        self.upgrade = None;
        self.stream = None;
        self.source_seekable = true;
        self.player.stop();
        self.loaded_key = None;
        self.duration = Duration::ZERO;
    }

    pub fn toggle_pause(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    /// True once the loaded track has played to its end.
    pub fn is_finished(&self) -> bool {
        self.loaded_key.is_some() && self.player.empty()
    }

    pub fn is_playing(&self) -> bool {
        self.loaded_key.is_some() && !self.player.empty() && !self.player.is_paused()
    }

    pub fn loaded_key(&self) -> Option<&str> {
        self.loaded_key.as_deref()
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn position(&self) -> Duration {
        // While a decoder is being built nothing is playing, and the player's
        // own position still refers to the track that was just stopped.
        if self.pending.is_some() {
            return Duration::ZERO;
        }
        clamp_position(self.player.get_pos(), self.duration)
    }

    pub fn progress(&self) -> f32 {
        progress_ratio(self.position(), self.duration)
    }

    /// Move the play head, without ever blocking the calling thread.
    ///
    /// Three things can happen, because a segment that is still arriving is
    /// played through an *unseekable* decoder (that is what lets it start at
    /// all — see [`PrepareJob::Streaming::seekable`]):
    ///
    /// * The installed decoder is seekable (a cached file, or one that has been
    ///   upgraded), so the jump happens now.
    /// * It is not, and the target is ahead of the play head within the buffered
    ///   region: symphonia emulates that by ignoring the bytes in between, so it
    ///   also happens now.
    /// * It is not, and the target is *behind* the play head. Nothing can make an
    ///   unseekable decoder go back, so if the file has finished downloading we
    ///   open a seekable view of it and jump when that lands. While it is still
    ///   arriving there is genuinely nothing to do, and the request is refused
    ///   rather than left to block on bytes that are not there yet.
    ///
    /// `Player::try_seek` waits for the decoder to move, which for an unseekable
    /// source reading past the download frontier would mean blocking on the
    /// network, so every path here is checked before it is taken.
    pub fn seek(&mut self, position: Duration) -> SeekOutcome {
        if self.pending.is_some() || self.upgrade.is_some() || self.loaded_key.is_none() {
            return SeekOutcome::Refused;
        }
        let target = clamp_position(position, self.duration);

        if self.source_seekable {
            return match self.player.try_seek(target) {
                Ok(()) => SeekOutcome::Seeked,
                Err(_) => SeekOutcome::Refused,
            };
        }

        // Unseekable: forward jumps inside the buffer are emulated by the reader.
        let buffered = self
            .stream
            .as_ref()
            .map_or(1.0, |state| state.buffered_fraction() as f64);
        let frontier = self.duration.mul_f64(buffered * 0.9);
        if target >= self.player.get_pos() && target <= frontier {
            return match self.player.try_seek(target) {
                Ok(()) => SeekOutcome::Seeked,
                Err(_) => SeekOutcome::Refused,
            };
        }

        if self.stream_is_complete() && !self.path.as_os_str().is_empty() {
            self.upgrade = Some(Upgrade {
                rx: spawn_prepare(PrepareJob::Complete(self.seekable_source())),
                target,
            });
            return SeekOutcome::Upgrading;
        }
        SeekOutcome::Refused
    }

    pub fn seek_fraction(&mut self, fraction: f32) -> SeekOutcome {
        let fraction = fraction.clamp(0.0, 1.0);
        let target = self.duration.mul_f32(fraction);
        self.seek(target)
    }

    /// Has every expected byte of the current track's segment landed?
    ///
    /// Deliberately not `!is_running()`: a failed or cancelled download leaves a
    /// truncated file, which must never be re-opened as if it were whole.
    fn stream_is_complete(&self) -> bool {
        self.stream
            .as_ref()
            .is_some_and(|state| state.status() == StreamStatus::Finished)
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.player.set_volume(self.volume);
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn clamps_position_to_duration() {
        let duration = Duration::from_secs(100);
        assert_eq!(
            clamp_position(Duration::from_secs(30), duration),
            Duration::from_secs(30)
        );
        assert_eq!(clamp_position(Duration::from_secs(300), duration), duration);
    }

    #[test]
    fn computes_progress_ratio() {
        let duration = Duration::from_secs(100);
        assert!((progress_ratio(Duration::from_secs(50), duration) - 0.5).abs() < 1e-6);
        assert!((progress_ratio(Duration::from_secs(0), duration)).abs() < 1e-6);
        assert!((progress_ratio(Duration::from_secs(100), duration) - 1.0).abs() < 1e-6);
        // Over-run is clamped rather than reported as >1.
        assert!((progress_ratio(Duration::from_secs(500), duration) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_duration_does_not_divide_by_zero() {
        assert_eq!(progress_ratio(Duration::from_secs(5), Duration::ZERO), 0.0);
    }

    /// A file that starts like an ISO-BMFF segment and then declares a `moov`
    /// box far larger than what exists on disk, so any decoder must read past
    /// everything published and therefore block on the streaming reader.
    fn blocking_segment() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&24u32.to_be_bytes());
        bytes.extend_from_slice(b"ftyp");
        bytes.extend_from_slice(b"isom");
        bytes.extend_from_slice(&0x200u32.to_be_bytes());
        bytes.extend_from_slice(b"isomiso2");
        bytes.extend_from_slice(&0x0100_0000u32.to_be_bytes()); // moov claims 16 MiB
        bytes.extend_from_slice(b"moov");
        bytes
    }

    /// Regression: the flag that decides whether playback can start early.
    ///
    /// Symphonia scans every atom — the whole file — when the source claims to
    /// be seekable, so a segment that is still arriving must never be opened
    /// that way. Measured on a 181.9 MB segment: seekable took 3.6 s and waited
    /// for 100% of the bytes; unseekable took 1.1 ms after 30 KB.
    #[test]
    fn a_still_arriving_segment_is_opened_unseekable() {
        let state = Arc::new(StreamState::new(1024));
        let source = PlaybackSource::Streaming {
            path: PathBuf::from("/tmp/listenbli-does-not-exist.m4s"),
            total: 1024,
            state,
        };
        match PrepareJob::streaming(&source).expect("a streaming source yields a job") {
            PrepareJob::Streaming { seekable, .. } => assert!(
                !seekable,
                "an unfinished segment must be opened unseekable or it waits for all of it"
            ),
            PrepareJob::Complete(_) => {
                panic!("a streaming source must not be opened as a complete file")
            }
        }

        // A file already on disk keeps full seeking; it costs a local scan.
        let complete = PlaybackSource::Complete(PathBuf::from("/tmp/listenbli.m4s"));
        assert!(
            PrepareJob::streaming(&complete).is_none(),
            "a cached file must take the seekable path"
        );
    }

    /// Regression: opening a decoder used to happen inline, and symphonia reads
    /// the *whole* segment while building its tables, so a click froze the window
    /// for the length of the download. Queueing it must cost nothing.
    #[test]
    fn queueing_a_decoder_never_blocks_the_caller() {
        let path = std::env::temp_dir().join(format!(
            "listenbli-engine-{}-{:?}.m4s",
            std::process::id(),
            std::thread::current().id()
        ));
        let bytes = blocking_segment();
        std::fs::write(&path, &bytes).unwrap();

        let total = 16 * 1024 * 1024;
        let state = Arc::new(StreamState::new(total));
        state.publish(bytes.len() as u64);

        let started = Instant::now();
        let rx = spawn_prepare(PrepareJob::Streaming {
            path: path.clone(),
            total,
            state: Arc::clone(&state),
            // What a still-arriving segment is really opened with.
            seekable: false,
        });
        let handoff = started.elapsed();
        assert!(
            handoff < Duration::from_millis(50),
            "queueing must not wait on the decoder: took {handoff:?}"
        );

        let outcome = match rx.try_recv() {
            // Parked on a read that cannot be satisfied — the case this guards.
            Err(TryRecvError::Empty) => {
                // Cancelling the stream is what the download worker does when a
                // newer track supersedes this one; it must end the job promptly
                // rather than leave the thread parked until the read times out.
                state.cancel();
                rx.recv_timeout(Duration::from_secs(5))
                    .expect("a cancelled stream must resolve the job")
            }
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => panic!("the prepare thread vanished"),
        };
        assert!(outcome.is_err(), "this segment cannot decode successfully");

        let _ = std::fs::remove_file(&path);
    }
}

//! Playback engine: a thin safe wrapper over rodio's `Player`.
//!
//! The M0 spike established two things this wrapper exists to absorb:
//! * `Decoder::total_duration()` is `None` for Bilibili's DASH fMP4 segments,
//!   so the duration always has to come from the API; and
//! * `Player::try_seek` cannot saturate at the end when the source does not
//!   report a duration, so positions must be clamped by us.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use rodio::decoder::DecoderBuilder;
use rodio::{Decoder, MixerDeviceSink, Player, Source};

use super::stream::{BlockingFileReader, PlaybackSource, StreamState};

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
    },
}

impl PrepareJob {
    fn of(source: &PlaybackSource) -> Self {
        match source {
            PlaybackSource::Complete(path) => PrepareJob::Complete(path.clone()),
            PlaybackSource::Streaming { path, total, state } => PrepareJob::Streaming {
                path: path.clone(),
                total: *total,
                state: Arc::clone(state),
            },
        }
    }
}

/// Open a decoder. **Blocking, and expensive — never call this on the UI thread.**
///
/// Symphonia's ISO-BMFF reader walks the whole segment to build its sample
/// tables, so this does not return until it has read the file to the end.
/// Measured against real segments: 100.00% of both a 4.7 MB and a 182 MB file is
/// read before `build()` returns; the small one takes 10 ms of CPU and the large
/// one is bounded only by how fast it arrives. Called inline it froze the window
/// for the length of the download, which is the whole reason this runs on a
/// thread.
fn build_source(job: PrepareJob) -> Result<PreparedSource, String> {
    match job {
        PrepareJob::Complete(path) => {
            let file = std::fs::File::open(&path)
                .map_err(|e| format!("无法打开音频缓存 {}: {e}", path.display()))?;
            let decoder = Decoder::try_from(file)
                .map_err(|e| format!("解码失败（音频格式可能不受支持）: {e}"))?;
            Ok(Box::new(decoder))
        }
        PrepareJob::Streaming { path, total, state } => {
            let reader = BlockingFileReader::open(&path, state)
                .map_err(|e| format!("无法打开音频缓存 {}: {e}", path.display()))?;
            let decoder = DecoderBuilder::new()
                .with_data(reader)
                .with_byte_len(total)
                .with_seekable(true)
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
        })
    }

    /// Hand a track to the player, returning immediately.
    ///
    /// Decoding is queued to a worker thread and lands via [`AudioEngine::poll`].
    /// Nothing here blocks: the window must stay responsive while a segment is
    /// being read, which for a long upload is the whole download.
    ///
    /// Playback of the previous track stops now, because the new one cannot
    /// start until its segment has been fully read and letting the old song
    /// continue under a new title would misrepresent what is happening.
    pub fn play_stream(
        &mut self,
        source: &PlaybackSource,
        duration: Duration,
        key: impl Into<String>,
    ) {
        let job = PrepareJob::of(source);
        self.pending = Some(Pending {
            rx: spawn_prepare(job),
            duration,
            key: key.into(),
            stream: source.stream_state().cloned(),
        });
        self.player.stop();
        self.loaded_key = None;
        self.stream = None;
        self.duration = duration;
    }

    /// Install a decoder that finished building.
    ///
    /// `None` while the newest selection is still being prepared; `Some(Ok(()))`
    /// once it is playing; `Some(Err(_))` if it could not be opened.
    pub fn poll(&mut self) -> Option<Result<(), String>> {
        let pending = self.pending.as_ref()?;
        match pending.rx.try_recv() {
            Ok(Ok(source)) => {
                let Pending {
                    duration,
                    key,
                    stream,
                    ..
                } = self.pending.take().expect("checked just above");
                self.install(source, duration, key, stream);
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
    ) {
        self.player.stop();
        self.duration = duration;
        self.stream = stream;
        self.player.append(source);
        self.player.set_volume(self.volume);
        self.player.play();
        self.loaded_key = Some(key);
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
        self.stream = None;
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

    pub fn seek(&self, position: Duration) -> Result<(), String> {
        let target = clamp_position(position, self.duration);
        self.player
            .try_seek(target)
            .map_err(|e| format!("跳转失败: {e:?}"))
    }

    pub fn seek_fraction(&self, fraction: f32) -> Result<(), String> {
        let fraction = fraction.clamp(0.0, 1.0);
        let target = self.duration.mul_f32(fraction);
        self.seek(target)
    }

    /// Whether a seek to `target` can complete without waiting on the download.
    ///
    /// `Player::try_seek` blocks the calling thread until the decoder has moved,
    /// and for a streaming source that means blocking on the blocking reader
    /// when the target lies beyond the download frontier. Since seeking happens
    /// on the UI thread, callers must check this first and refuse (or defer)
    /// rather than freeze the window.
    pub fn can_seek_without_waiting(&self, target: Duration) -> bool {
        let Some(state) = &self.stream else {
            return true;
        };
        if !state.is_running() {
            // Complete, failed or cancelled: reads no longer block (or fail fast).
            return true;
        }
        // AAC is roughly constant bitrate, so bytes map proportionally to time.
        let buffered = self.duration.mul_f64(state.buffered_fraction() as f64);
        // Leave a margin so we never land right on the frontier.
        target <= buffered.mul_f64(0.9)
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

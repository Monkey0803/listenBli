//! Playback engine: a thin safe wrapper over rodio's `Player`.
//!
//! The M0 spike established two things this wrapper exists to absorb:
//! * `Decoder::total_duration()` is `None` for Bilibili's DASH fMP4 segments,
//!   so the duration always has to come from the API; and
//! * `Player::try_seek` cannot saturate at the end when the source does not
//!   report a duration, so positions must be clamped by us.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::decoder::DecoderBuilder;
use rodio::{Decoder, MixerDeviceSink, Player};

use super::stream::{BlockingFileReader, PlaybackSource, StreamState};

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
        })
    }

    /// Play a fully cached file.
    pub fn play(
        &mut self,
        path: &Path,
        duration: Duration,
        key: impl Into<String>,
    ) -> Result<(), String> {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("无法打开音频缓存 {}: {e}", path.display()))?;
        let decoder = Decoder::try_from(file)
            .map_err(|e| format!("解码失败（音频格式可能不受支持）: {e}"))?;
        self.start(decoder, duration, key, None);
        Ok(())
    }

    /// Play a segment that is still downloading.
    ///
    /// The decoder is told the segment's *real* final size, and its reads block
    /// until the bytes arrive, so playback starts after only the first few
    /// hundred kilobytes instead of the whole song.
    pub fn play_stream(
        &mut self,
        source: &PlaybackSource,
        duration: Duration,
        key: impl Into<String>,
    ) -> Result<(), String> {
        let key = key.into();
        match source {
            PlaybackSource::Complete(path) => self.play(path, duration, key),
            PlaybackSource::Streaming { path, total, state } => {
                let reader = BlockingFileReader::open(path, Arc::clone(state))
                    .map_err(|e| format!("无法打开音频缓存 {}: {e}", path.display()))?;
                let decoder = DecoderBuilder::new()
                    .with_data(reader)
                    .with_byte_len(*total)
                    .with_seekable(true)
                    .build()
                    .map_err(|e| format!("解码失败（音频格式可能不受支持）: {e}"))?;
                self.start(decoder, duration, key, Some(Arc::clone(state)));
                Ok(())
            }
        }
    }

    fn start<R>(
        &mut self,
        decoder: Decoder<R>,
        duration: Duration,
        key: impl Into<String>,
        stream: Option<Arc<StreamState>>,
    ) where
        R: std::io::Read + std::io::Seek + Send + 'static,
    {
        self.player.stop();
        self.duration = duration;
        self.stream = stream;
        self.player.append(decoder);
        self.player.set_volume(self.volume);
        self.player.play();
        self.loaded_key = Some(key.into());
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
}

//! Streaming playback support: a `Read + Seek` view over a file that is still
//! being downloaded.
//!
//! # Why this exists
//!
//! Bilibili serves DASH audio as a fragmented MP4 (`ftyp` + `moov` + `sidx` +
//! 54 `moof`/`mdat` fragments). Naively downloading into a file and appending
//! while playing does **not** work: rodio fixes the stream length when the
//! decoder is opened, so the end of a partially written file is terminal.
//! `examples/spike_streaming.rs` demonstrates this — appending 5.99 MB after the
//! decoder opened bought exactly zero extra audio.
//!
//! The fix is to tell the decoder the *real* total length up front (the CDN
//! sends `Content-Length`) and make `read` **block** until the requested bytes
//! have actually been written. Reads past the download frontier then wait
//! instead of reporting a premature end-of-file, so playback starts after only
//! the first few hundred kilobytes.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a blocked read waits before giving up.
///
/// This is a safety valve, not a normal path: the download completes in seconds
/// and unblocks every reader. It exists so a dead download thread can never
/// wedge the audio thread (or a seek on the UI thread) forever.
pub const READ_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamStatus {
    Running,
    /// Every expected byte has been written.
    Finished,
    Failed(String),
    Cancelled,
}

#[derive(Debug)]
struct Inner {
    /// Bytes durably written so far. Monotonic.
    written: u64,
    total: u64,
    status: StreamStatus,
}

/// Shared state between the download thread (writer) and the decoder (reader).
#[derive(Debug)]
pub struct StreamState {
    inner: Mutex<Inner>,
    ready: Condvar,
}

impl StreamState {
    pub fn new(total: u64) -> Self {
        Self {
            inner: Mutex::new(Inner {
                written: 0,
                total,
                status: StreamStatus::Running,
            }),
            ready: Condvar::new(),
        }
    }

    pub fn total(&self) -> u64 {
        self.inner.lock().unwrap().total
    }

    pub fn written(&self) -> u64 {
        self.inner.lock().unwrap().written
    }

    pub fn status(&self) -> StreamStatus {
        self.inner.lock().unwrap().status.clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status(), StreamStatus::Running)
    }

    /// Fraction of the segment buffered, `0.0..=1.0`.
    pub fn buffered_fraction(&self) -> f32 {
        let inner = self.inner.lock().unwrap();
        if inner.total == 0 {
            return 0.0;
        }
        (inner.written as f64 / inner.total as f64).clamp(0.0, 1.0) as f32
    }

    /// Announce how many bytes are now readable. Must be called *after* the
    /// bytes have been handed to the OS, otherwise a reader can observe data
    /// that is not yet there.
    pub fn publish(&self, written: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.written = inner.written.max(written);
        drop(inner);
        self.ready.notify_all();
    }

    pub fn finish(&self) {
        let mut inner = self.inner.lock().unwrap();
        // Only a still-running stream can finish; do not overwrite a failure or
        // a cancellation with a success.
        if inner.status == StreamStatus::Running {
            inner.status = StreamStatus::Finished;
        }
        drop(inner);
        self.ready.notify_all();
    }

    pub fn fail(&self, message: impl Into<String>) {
        let mut inner = self.inner.lock().unwrap();
        if inner.status == StreamStatus::Running {
            inner.status = StreamStatus::Failed(message.into());
        }
        drop(inner);
        self.ready.notify_all();
    }

    pub fn cancel(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.status == StreamStatus::Running {
            inner.status = StreamStatus::Cancelled;
        }
        drop(inner);
        self.ready.notify_all();
    }

    /// Block until more than `offset` bytes are available.
    ///
    /// Returns the number of readable bytes starting at `offset`; `0` means the
    /// stream is genuinely finished at this offset.
    fn wait_available(&self, offset: u64, timeout: Duration) -> io::Result<u64> {
        let deadline = Instant::now() + timeout;
        let mut inner = self.inner.lock().unwrap();

        loop {
            if inner.written > offset {
                return Ok(inner.written - offset);
            }
            match &inner.status {
                StreamStatus::Finished => {
                    // The real end of the stream.
                    return Ok(0);
                }
                StreamStatus::Failed(message) => {
                    return Err(io::Error::other(format!("音频下载失败: {message}")));
                }
                StreamStatus::Cancelled => {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "音频下载已取消"));
                }
                StreamStatus::Running => {}
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "等待音频数据超时（网络过慢或下载已中断）",
                ));
            }
            let (guard, _) = self
                .ready
                .wait_timeout(inner, deadline - now)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            inner = guard;
        }
    }
}

/// Where a ready track's audio comes from.
#[derive(Debug, Clone)]
pub enum PlaybackSource {
    /// Fully present on disk: ordinary, non-blocking playback.
    Complete(PathBuf),
    /// Still arriving: reads block until the bytes land.
    Streaming {
        path: PathBuf,
        total: u64,
        state: Arc<StreamState>,
    },
}

impl PlaybackSource {
    pub fn path(&self) -> &Path {
        match self {
            PlaybackSource::Complete(path) => path,
            PlaybackSource::Streaming { path, .. } => path,
        }
    }

    pub fn stream_state(&self) -> Option<&Arc<StreamState>> {
        match self {
            PlaybackSource::Complete(_) => None,
            PlaybackSource::Streaming { state, .. } => Some(state),
        }
    }
}

/// `Read + Seek` over a file that a download thread is still growing.
///
/// `total` is the *real* final size, reported by `seek(SeekFrom::End(0))` and
/// used as the decoder's byte length, so the decoder never mistakes the current
/// download frontier for the end of the stream.
pub struct BlockingFileReader {
    file: File,
    state: Arc<StreamState>,
    position: u64,
}

impl BlockingFileReader {
    pub fn open(path: &Path, state: Arc<StreamState>) -> io::Result<Self> {
        Ok(Self {
            file: File::open(path)?,
            state,
            position: 0,
        })
    }
}

impl Read for BlockingFileReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let total = self.state.total();
        // Past the real end of the stream: a genuine, non-blocking EOF.
        if self.position >= total {
            return Ok(0);
        }

        let available = self.state.wait_available(self.position, READ_TIMEOUT)?;
        if available == 0 {
            return Ok(0);
        }

        let want = buf.len().min(available as usize);
        self.file.seek(SeekFrom::Start(self.position))?;
        let read = self.file.read(&mut buf[..want])?;
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for BlockingFileReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        // Seeking never blocks: only reads wait for data. The decoder seeks a
        // lot while probing and building its tables.
        let total = self.state.total() as i64;
        let target = match position {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => total + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "试图定位到负数偏移",
            ));
        }
        self.position = target as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("listenbli-stream-{tag}-{}.bin", std::process::id()))
    }

    #[test]
    fn reports_the_real_total_length() {
        let state = Arc::new(StreamState::new(1000));
        let path = temp_path("total");
        std::fs::write(&path, b"").unwrap();
        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), 1000);
        assert_eq!(reader.stream_position().unwrap(), 1000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_blocks_until_data_is_published() {
        let state = Arc::new(StreamState::new(8));
        let path = temp_path("blocking");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"abcd").unwrap();
        file.sync_all().unwrap();

        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        state.publish(4);

        // The first four bytes are available immediately.
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"abcd");

        // The next byte is not: the reader must wait for the publisher.
        let writer_state = Arc::clone(&state);
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&writer_path)
                .unwrap();
            file.write_all(b"efgh").unwrap();
            file.sync_all().unwrap();
            writer_state.publish(8);
            writer_state.finish();
        });

        let started = Instant::now();
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        let waited = started.elapsed();

        assert_eq!(rest, b"efgh");
        assert!(
            waited >= Duration::from_millis(80),
            "read returned before the data was published ({waited:?})"
        );
        writer.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn finished_stream_terminates_instead_of_hanging() {
        let state = Arc::new(StreamState::new(4));
        let path = temp_path("finished");
        std::fs::write(&path, b"abcd").unwrap();
        state.publish(4);
        state.finish();

        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        let mut all = Vec::new();
        reader.read_to_end(&mut all).unwrap();
        assert_eq!(all, b"abcd");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_failed_download_surfaces_as_an_io_error() {
        let state = Arc::new(StreamState::new(100));
        let path = temp_path("failed");
        std::fs::write(&path, b"ab").unwrap();
        state.publish(2);
        state.fail("CDN 403");

        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        let mut buf = [0u8; 16];
        reader.read_exact(&mut buf[..2]).unwrap();
        let err = reader.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(err.to_string().contains("403"), "got {err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_cancelled_download_surfaces_as_interrupted() {
        let state = Arc::new(StreamState::new(100));
        let path = temp_path("cancelled");
        std::fs::write(&path, b"ab").unwrap();
        state.publish(2);
        state.cancel();

        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        let mut buf = [0u8; 16];
        reader.read_exact(&mut buf[..2]).unwrap();
        let err = reader.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_stalled_download_times_out_instead_of_hanging_forever() {
        let state = Arc::new(StreamState::new(1000));
        let path = temp_path("stalled");
        std::fs::write(&path, b"ab").unwrap();
        state.publish(2);

        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        // Wait directly with a short timeout rather than the 15s constant.
        let started = Instant::now();
        let err = state
            .wait_available(2, Duration::from_millis(150))
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
        let _ = reader.seek(SeekFrom::Start(0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reads_across_a_growing_file_from_any_offset() {
        let state = Arc::new(StreamState::new(10));
        let path = temp_path("offsets");
        std::fs::write(&path, b"0123456789").unwrap();
        state.publish(10);
        state.finish();

        let mut reader = BlockingFileReader::open(&path, Arc::clone(&state)).unwrap();
        reader.seek(SeekFrom::Start(4)).unwrap();
        let mut buf = [0u8; 3];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"456");
        assert_eq!(reader.seek(SeekFrom::Current(-2)).unwrap(), 5);
        // Seeking past the end then reading yields a clean EOF, not an error.
        reader.seek(SeekFrom::Start(9999)).unwrap();
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn buffered_fraction_tracks_published_bytes() {
        let state = StreamState::new(200);
        assert_eq!(state.buffered_fraction(), 0.0);
        state.publish(50);
        assert!((state.buffered_fraction() - 0.25).abs() < 1e-6);
        state.publish(200);
        assert!((state.buffered_fraction() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn publish_never_goes_backwards() {
        let state = StreamState::new(1000);
        state.publish(500);
        state.publish(100);
        assert_eq!(state.written(), 500);
    }

    /// Terminal states are sticky: a late `finish`/`fail` must not rewrite the
    /// outcome the reader already observed.
    #[test]
    fn terminal_states_are_not_overwritten() {
        let failed = StreamState::new(10);
        failed.fail("boom");
        failed.finish();
        assert!(matches!(failed.status(), StreamStatus::Failed(_)));

        let cancelled = StreamState::new(10);
        cancelled.cancel();
        cancelled.fail("late");
        assert_eq!(cancelled.status(), StreamStatus::Cancelled);

        let finished = StreamState::new(10);
        finished.publish(10);
        finished.finish();
        finished.fail("late");
        assert_eq!(finished.status(), StreamStatus::Finished);
    }
}

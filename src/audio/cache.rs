//! On-disk cache for downloaded audio segments.
//!
//! Bilibili's DASH audio URLs are time-limited and IP-bound, so we keep the
//! segment once fetched. A song is only a few megabytes.
//!
//! A segment may be *partially* present while it streams, so each entry carries
//! a sidecar recording the expected total size. Only a file whose length matches
//! that total counts as a cache hit — otherwise an interrupted download would
//! later be handed to the decoder as if it were a complete song.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::models::AudioQuality;
use crate::platform;

/// Keep the cache under this size, evicting least-recently-modified files first.
const MAX_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// An ISO-BMFF file starts with a box size then `ftyp` at offset 4.
///
/// The CDN answers some failures with an HTML error page and a 200 status, so
/// structural validation matters more than the HTTP status code.
pub fn looks_like_iso_bmff(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[4..8] == b"ftyp"
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheMeta {
    /// Size the completed segment is expected to have, from `Content-Length`.
    pub total: u64,
}

pub struct AudioCache {
    dir: PathBuf,
}

impl AudioCache {
    pub fn new() -> Self {
        Self {
            dir: platform::cache_dir().join("audio"),
        }
    }

    #[cfg(test)]
    pub fn with_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path_for(&self, cid: i64, quality: AudioQuality) -> PathBuf {
        self.dir.join(format!("{cid}_{}.m4s", quality.stream_id()))
    }

    fn meta_path_for(&self, cid: i64, quality: AudioQuality) -> PathBuf {
        self.dir
            .join(format!("{cid}_{}.meta.json", quality.stream_id()))
    }

    /// Record the expected size before/while streaming, so a partial file is
    /// never mistaken for a complete one.
    pub fn begin(&self, cid: i64, quality: AudioQuality, total: u64) -> std::io::Result<()> {
        platform::ensure_dir(&self.dir)?;
        let meta = CacheMeta { total };
        let text = serde_json::to_vec(&meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Write the sidecar first: a `.m4s` without a sidecar is never a hit.
        std::fs::write(self.meta_path_for(cid, quality), text)
    }

    /// A cached segment that is complete *and* structurally valid.
    pub fn get(&self, cid: i64, quality: AudioQuality) -> Option<PathBuf> {
        let path = self.path_for(cid, quality);

        // A `.m4s` without a sidecar is a leftover from an interrupted attempt
        // (or from an older version of this app): never a hit, and worth
        // cleaning up so the cache does not accumulate junk.
        let Some(expected) = self.read_meta(cid, quality) else {
            if path.exists() {
                self.remove(cid, quality);
            }
            return None;
        };

        let actual = std::fs::metadata(&path).ok()?.len();
        if actual != expected.total {
            // Partial (interrupted download) or truncated: drop it.
            self.remove(cid, quality);
            return None;
        }

        let head = read_head(&path, 16)?;
        if looks_like_iso_bmff(&head) {
            Some(path)
        } else {
            // An error page saved as `.m4s`: drop it so the next attempt refetches.
            self.remove(cid, quality);
            None
        }
    }

    /// True when a complete segment is already on disk.
    pub fn is_complete(&self, cid: i64, quality: AudioQuality) -> bool {
        self.get(cid, quality).is_some()
    }

    fn read_meta(&self, cid: i64, quality: AudioQuality) -> Option<CacheMeta> {
        let text = std::fs::read_to_string(self.meta_path_for(cid, quality)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Delete a segment and its sidecar.
    pub fn remove(&self, cid: i64, quality: AudioQuality) {
        let _ = std::fs::remove_file(self.path_for(cid, quality));
        let _ = std::fs::remove_file(self.meta_path_for(cid, quality));
    }

    pub fn store(&self, cid: i64, quality: AudioQuality, bytes: &[u8]) -> std::io::Result<PathBuf> {
        platform::ensure_dir(&self.dir)?;
        let path = self.path_for(cid, quality);
        std::fs::write(&path, bytes)?;
        self.begin(cid, quality, bytes.len() as u64)?;
        Ok(path)
    }

    /// Total size of cached segments, in bytes.
    pub fn total_bytes(&self) -> u64 {
        self.entries().iter().map(|(_, size, _)| *size).sum()
    }

    /// `(path, size, modified)` for every cached segment.
    fn entries(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let Ok(read_dir) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        read_dir
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("m4s") {
                    return None;
                }
                let meta = entry.metadata().ok()?;
                if !meta.is_file() {
                    return None;
                }
                let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                Some((path, meta.len(), modified))
            })
            .collect()
    }

    /// Delete oldest-first until the cache fits under `max_bytes`.
    /// Returns the number of files removed.
    pub fn evict_to_fit(&self) -> usize {
        self.evict(MAX_CACHE_BYTES)
    }

    pub fn evict(&self, max_bytes: u64) -> usize {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|(_, size, _)| *size).sum();
        if total <= max_bytes {
            return 0;
        }
        // Oldest modification time first.
        entries.sort_by_key(|(_, _, modified)| *modified);
        let mut removed = 0;
        for (path, size, _) in entries {
            if total <= max_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
                removed += 1;
            }
            // Drop the sidecar alongside the segment.
            let sidecar = path.with_extension("meta.json");
            let _ = std::fs::remove_file(sidecar);
        }
        removed
    }
}

impl Default for AudioCache {
    fn default() -> Self {
        Self::new()
    }
}

fn read_head(path: &Path, len: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; len];
    let read = file.read(&mut buf).ok()?;
    buf.truncate(read);
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("listenbli-cache-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fake_segment(payload: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; 4];
        bytes.extend_from_slice(b"ftyp");
        bytes.resize(8 + payload, 0);
        bytes
    }

    #[test]
    fn detects_container_signature() {
        assert!(looks_like_iso_bmff(&fake_segment(64)));
        assert!(!looks_like_iso_bmff(b"<!DOCTYPE html><html>"));
        assert!(!looks_like_iso_bmff(b"short"));
        assert!(!looks_like_iso_bmff(&[]));
    }

    #[test]
    fn stores_and_reads_back() {
        let dir = temp_dir("roundtrip");
        let cache = AudioCache::with_dir(dir.clone());
        assert!(cache.get(42, AudioQuality::K192).is_none());

        let path = cache
            .store(42, AudioQuality::K192, &fake_segment(128))
            .unwrap();
        assert_eq!(path, cache.path_for(42, AudioQuality::K192));
        assert_eq!(cache.get(42, AudioQuality::K192), Some(path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_and_deletes_html_error_pages() {
        let dir = temp_dir("html");
        let cache = AudioCache::with_dir(dir.clone());
        let path = cache.path_for(7, AudioQuality::K132);
        std::fs::write(&path, b"<html>403 forbidden</html>").unwrap();
        assert!(cache.get(7, AudioQuality::K132).is_none());
        assert!(!path.exists(), "poisoned cache entry should be removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn distinct_qualities_do_not_collide() {
        let dir = temp_dir("qualities");
        let cache = AudioCache::with_dir(dir.clone());
        assert_ne!(
            cache.path_for(1, AudioQuality::K192),
            cache.path_for(1, AudioQuality::K132)
        );
        assert_ne!(
            cache.path_for(1, AudioQuality::K192),
            cache.path_for(2, AudioQuality::K192)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn evicts_oldest_until_under_limit() {
        let dir = temp_dir("evict");
        let cache = AudioCache::with_dir(dir.clone());
        for i in 0..4 {
            let path = dir.join(format!("{i}_30280.m4s"));
            std::fs::write(&path, fake_segment(1000)).unwrap();
            // Stagger mtimes so ordering is deterministic.
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let total = cache.total_bytes();
        assert!(total > 2000);

        let removed = cache.evict(total / 2);
        assert!(removed >= 1, "expected at least one eviction");
        assert!(cache.total_bytes() <= total / 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn evict_is_a_noop_when_under_limit() {
        let dir = temp_dir("noop");
        let cache = AudioCache::with_dir(dir.clone());
        cache
            .store(1, AudioQuality::K192, &fake_segment(100))
            .unwrap();
        assert_eq!(cache.evict(u64::MAX), 0);
        assert_eq!(cache.evict_to_fit(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A segment that is still streaming (or an interrupted download) must never
    /// be treated as a complete cache hit, otherwise the decoder is handed a
    /// truncated song.
    #[test]
    fn a_partial_segment_is_not_a_cache_hit() {
        let dir = temp_dir("partial");
        let cache = AudioCache::with_dir(dir.clone());

        let segment = fake_segment(2000);
        cache
            .begin(9, AudioQuality::K192, segment.len() as u64)
            .unwrap();

        // Only half of it has landed so far.
        let path = cache.path_for(9, AudioQuality::K192);
        std::fs::write(&path, &segment[..1000]).unwrap();
        assert!(!cache.is_complete(9, AudioQuality::K192));
        assert!(cache.get(9, AudioQuality::K192).is_none());
        assert!(!path.exists(), "the partial file should be discarded");

        // Once the whole segment is present it becomes a hit.
        cache.store(9, AudioQuality::K192, &segment).unwrap();
        assert!(cache.is_complete(9, AudioQuality::K192));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_segment_without_a_sidecar_is_not_a_hit() {
        let dir = temp_dir("nometa");
        let cache = AudioCache::with_dir(dir.clone());
        // Written directly, as an interrupted first attempt would leave it.
        let path = cache.path_for(3, AudioQuality::K192);
        std::fs::write(&path, fake_segment(500)).unwrap();
        assert!(cache.get(3, AudioQuality::K192).is_none());
        assert!(!path.exists(), "the orphaned file should be cleaned up");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn begin_does_not_make_an_absent_file_a_hit() {
        let dir = temp_dir("beginonly");
        let cache = AudioCache::with_dir(dir.clone());
        cache.begin(5, AudioQuality::K132, 4096).unwrap();
        assert!(cache.get(5, AudioQuality::K132).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn eviction_also_removes_the_sidecar() {
        let dir = temp_dir("evictmeta");
        let cache = AudioCache::with_dir(dir.clone());
        cache
            .store(1, AudioQuality::K192, &fake_segment(2000))
            .unwrap();
        assert!(dir.join("1_30280.meta.json").exists());

        cache.evict(0);
        assert!(!cache.path_for(1, AudioQuality::K192).exists());
        assert!(
            !dir.join("1_30280.meta.json").exists(),
            "the sidecar must be cleaned up too"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_deletes_both_files() {
        let dir = temp_dir("remove");
        let cache = AudioCache::with_dir(dir.clone());
        cache
            .store(2, AudioQuality::K192, &fake_segment(64))
            .unwrap();
        cache.remove(2, AudioQuality::K192);
        assert!(cache.get(2, AudioQuality::K192).is_none());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

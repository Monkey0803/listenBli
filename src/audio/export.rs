//! Turn a cached DASH segment into an ordinary audio file.
//!
//! Bilibili serves audio as fragmented MP4: an init segment (`ftyp` + `moov`
//! carrying the AAC decoder configuration, with an empty `stbl` and a `mvex`
//! promising that the samples will arrive later) followed by `moof`/`mdat` pairs,
//! each `moof` describing the samples in the `mdat` behind it. That is a legal
//! ISO-BMFF file, but it is not what anything else expects: no Finder preview, no
//! QuickTime, and an extension (`.m4s`) that says "streaming fragment".
//!
//! So this rewrites one into a *progressive* MP4: every sample's size and duration
//! is collected from the `moof` boxes into a real `stbl`, the sample payloads are
//! concatenated into a single `mdat`, and the durations are patched into
//! `mvhd`/`tkhd`/`mdhd`. The AAC bytes themselves are copied untouched — this is a
//! remux, not a transcode, so it is lossless, costs one pass of I/O, and needs no
//! encoder.
//!
//! Everything here works on one cached file at a time and never buffers the whole
//! thing in memory: the sample *payloads* are streamed straight from the input to
//! the output, and only the boxes (a few kilobytes) are held.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Bytes of box header: a 32-bit size and a four-character type.
const HEADER: u64 = 8;

/// One sample as the fragments describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sample {
    size: u32,
    duration: u32,
}

/// A box's type and payload, with containers left unparsed.
#[derive(Debug, Clone)]
struct RawBox {
    kind: [u8; 4],
    /// Everything after the 8-byte header, so `version`/`flags` included.
    payload: Vec<u8>,
}

impl RawBox {
    fn is(&self, kind: &[u8; 4]) -> bool {
        &self.kind == kind
    }

    fn children(&self) -> Result<Vec<RawBox>, String> {
        parse_boxes(&self.payload, &self.kind)
    }

    fn serialize(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&((self.payload.len() as u32) + 8).to_be_bytes());
        out.extend_from_slice(&self.kind);
        out.extend_from_slice(&self.payload);
    }
}

/// What a fragment's `traf` says about its samples.
#[derive(Debug, Default)]
struct TrackDefaults {
    duration: u32,
    size: u32,
}

/// The result of reading the init segment.
struct Init {
    moov: RawBox,
    /// Media timescale from `mdhd`.
    media_timescale: u32,
    /// Movie timescale from `mvhd`, which `tkhd` durations are also expressed in.
    movie_timescale: u32,
    defaults: TrackDefaults,
    /// The decoder configuration, copied verbatim into the new `stbl`.
    stsd: RawBox,
}

/// Read one box header, returning its type and payload length.
///
/// A 64-bit size (`size == 1`) is understood because the format allows it, even
/// though Bilibili's segments do not use it.
fn read_header<R: Read>(reader: &mut R, offset: u64) -> Result<([u8; 4], u64), String> {
    let mut header = [0u8; 8];
    reader
        .read_exact(&mut header)
        .map_err(|err| format!("读取盒子头失败（偏移 {offset}）：{err}"))?;
    let mut kind = [0u8; 4];
    kind.copy_from_slice(&header[4..8]);
    let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let size = match size32 {
        0 => return Err(format!("盒子 {} 长度为 0，无法定位下一个", fourcc(&kind))),
        1 => {
            let mut wide = [0u8; 8];
            reader
                .read_exact(&mut wide)
                .map_err(|err| format!("读取 64 位盒子长度失败：{err}"))?;
            u64::from_be_bytes(wide)
        }
        other => u64::from(other),
    };
    if size < HEADER {
        return Err(format!("盒子 {} 长度非法：{size}", fourcc(&kind)));
    }
    Ok((kind, size - if size32 == 1 { 16 } else { HEADER }))
}

/// Parse a run of boxes out of a payload.
fn parse_boxes(payload: &[u8], parent: &[u8; 4]) -> Result<Vec<RawBox>, String> {
    let mut boxes = Vec::new();
    let mut cursor = 0usize;
    while cursor + HEADER as usize <= payload.len() {
        let size = u32::from_be_bytes([
            payload[cursor],
            payload[cursor + 1],
            payload[cursor + 2],
            payload[cursor + 3],
        ]) as usize;
        let mut kind = [0u8; 4];
        kind.copy_from_slice(&payload[cursor + 4..cursor + 8]);
        if size < HEADER as usize || cursor + size > payload.len() {
            return Err(format!(
                "{} 里的 {} 长度非法（{size}）",
                fourcc(parent),
                fourcc(&kind)
            ));
        }
        boxes.push(RawBox {
            kind,
            payload: payload[cursor + HEADER as usize..cursor + size].to_vec(),
        });
        cursor += size;
    }
    Ok(boxes)
}

fn fourcc(kind: &[u8; 4]) -> String {
    String::from_utf8_lossy(kind).to_string()
}

/// The `n`th field of a big-endian payload, with the box version in mind.
fn u32_at(payload: &[u8], at: usize) -> Result<u32, String> {
    payload
        .get(at..at + 4)
        .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .ok_or_else(|| format!("盒子在偏移 {at} 处被截断"))
}

#[cfg(test)]
fn u64_at(payload: &[u8], at: usize) -> Result<u64, String> {
    payload
        .get(at..at + 8)
        .map(|bytes| {
            u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        })
        .ok_or_else(|| format!("盒子在偏移 {at} 处被截断"))
}

/// Where `duration` sits inside a full box, and how wide it is.
///
/// `mvhd`, `tkhd` and `mdhd` all carry creation/modification times before it, and
/// all three widen every one of those fields together in version 1 — so the
/// offset depends on the version, which is the first byte of the payload.
fn duration_field(payload: &[u8], after_timescale: usize) -> Result<(usize, bool), String> {
    let version = *payload.first().ok_or("盒子没有 version 字段")?;
    match version {
        0 => Ok((after_timescale + 4, false)),
        1 => Ok((after_timescale + 8, true)),
        other => Err(format!("不支持的盒子版本 {other}")),
    }
}

/// Patch a box's duration in place, leaving every other byte alone.
fn patch_duration(payload: &mut [u8], after_timescale: usize, duration: u64) -> Result<(), String> {
    let (at, wide) = duration_field(payload, after_timescale)?;
    if wide {
        let bytes = duration.to_be_bytes();
        payload
            .get_mut(at..at + 8)
            .ok_or("时长字段被截断")?
            .copy_from_slice(&bytes);
    } else {
        let clamped = u32::try_from(duration).unwrap_or(u32::MAX);
        let bytes = clamped.to_be_bytes();
        payload
            .get_mut(at..at + 4)
            .ok_or("时长字段被截断")?
            .copy_from_slice(&bytes);
    }
    Ok(())
}

/// Read the init segment: `ftyp` and `moov`, plus the numbers the rebuild needs.
fn read_init<R: Read + Seek>(reader: &mut R, end: u64) -> Result<Init, String> {
    let mut ftyp = None;
    let mut moov = None;
    let mut offset = 0u64;
    while offset + HEADER <= end {
        reader
            .seek(SeekFrom::Start(offset))
            .map_err(|err| format!("定位盒子失败：{err}"))?;
        let (kind, len) = read_header(reader, offset)?;
        if &kind == b"ftyp" || &kind == b"moov" {
            let mut payload = vec![0u8; len as usize];
            reader
                .read_exact(&mut payload)
                .map_err(|err| format!("读取 {} 失败：{err}", fourcc(&kind)))?;
            let raw = RawBox { kind, payload };
            if raw.is(b"ftyp") {
                ftyp = Some(raw);
            } else {
                moov = Some(raw);
            }
        }
        offset += len + HEADER;
        // The init segment is all we need; stop before the fragments.
        if &kind == b"moof" {
            break;
        }
    }

    ftyp.ok_or("这个文件没有 ftyp，可能不是 MP4")?;
    let moov = moov.ok_or("这个文件没有 moov，可能不是完整的缓存")?;

    let trak = moov
        .children()?
        .into_iter()
        .find(|child| child.is(b"trak"))
        .ok_or("moov 里没有 trak")?;
    let trak_children = trak.children()?;
    let mdia = trak_children
        .iter()
        .find(|child| child.is(b"mdia"))
        .ok_or("trak 里没有 mdia")?
        .children()?;
    let mdhd = mdia
        .iter()
        .find(|child| child.is(b"mdhd"))
        .ok_or("mdia 里没有 mdhd")?;
    let media_timescale = u32_at(&mdhd.payload, 4 + 8)?;
    let minf = mdia
        .iter()
        .find(|child| child.is(b"minf"))
        .ok_or("mdia 里没有 minf")?
        .children()?;
    let stbl = minf
        .iter()
        .find(|child| child.is(b"stbl"))
        .ok_or("minf 里没有 stbl")?
        .children()?;
    let stsd = stbl
        .into_iter()
        .find(|child| child.is(b"stsd"))
        .ok_or("stbl 里没有 stsd（缺少解码器配置）")?;

    let mvhd = moov
        .children()?
        .into_iter()
        .find(|child| child.is(b"mvhd"))
        .ok_or("moov 里没有 mvhd")?;
    let movie_timescale = u32_at(&mvhd.payload, 4 + 8)?;

    // `trex` carries the per-track defaults a fragment may leave out.
    let mut defaults = TrackDefaults::default();
    if let Some(mvex) = moov.children()?.into_iter().find(|child| child.is(b"mvex")) {
        if let Some(trex) = mvex.children()?.into_iter().find(|child| child.is(b"trex")) {
            // version/flags(4) track_id(4) default_sample_description_index(4)
            // default_sample_duration(4) default_sample_size(4)
            defaults.duration = u32_at(&trex.payload, 12)?;
            defaults.size = u32_at(&trex.payload, 16)?;
        }
    }

    Ok(Init {
        moov,
        media_timescale,
        movie_timescale,
        defaults,
        stsd,
    })
}

/// Collect every sample's size and duration, in order.
fn read_samples<R: Read + Seek>(
    reader: &mut R,
    start: u64,
    end: u64,
    init: &Init,
) -> Result<Vec<Sample>, String> {
    let mut samples = Vec::new();
    let mut offset = start;
    while offset + HEADER <= end {
        reader
            .seek(SeekFrom::Start(offset))
            .map_err(|err| format!("定位片段失败：{err}"))?;
        let (kind, len) = read_header(reader, offset)?;
        if &kind == b"moof" {
            let mut payload = vec![0u8; len as usize];
            reader
                .read_exact(&mut payload)
                .map_err(|err| format!("读取 moof 失败：{err}"))?;
            let moof = RawBox { kind, payload };
            for traf in moof
                .children()?
                .into_iter()
                .filter(|child| child.is(b"traf"))
            {
                read_traf(&traf, init, &mut samples)?;
            }
        }
        offset += len + HEADER;
    }
    if samples.is_empty() {
        return Err("这个文件里没有音频样本".to_owned());
    }
    Ok(samples)
}

/// Pull the sample sizes and durations out of one `traf`.
fn read_traf(traf: &RawBox, init: &Init, samples: &mut Vec<Sample>) -> Result<(), String> {
    let children = traf.children()?;
    let tfhd = children
        .iter()
        .find(|child| child.is(b"tfhd"))
        .ok_or("traf 里没有 tfhd")?;
    // version/flags(4) track_id(4) then optional fields in flag order.
    let flags = u32_at(&tfhd.payload, 0)? & 0x00ff_ffff;
    let mut cursor = 8usize;
    if flags & 0x000001 != 0 {
        cursor += 8; // base_data_offset
    }
    if flags & 0x000002 != 0 {
        cursor += 4; // sample_description_index
    }
    let mut default_duration = init.defaults.duration;
    if flags & 0x000008 != 0 {
        default_duration = u32_at(&tfhd.payload, cursor)?;
        cursor += 4;
    }
    let mut default_size = init.defaults.size;
    if flags & 0x000010 != 0 {
        default_size = u32_at(&tfhd.payload, cursor)?;
    }

    for trun in children.iter().filter(|child| child.is(b"trun")) {
        let trun_flags = u32_at(&trun.payload, 0)? & 0x00ff_ffff;
        let count = u32_at(&trun.payload, 4)?;
        let mut at = 8usize;
        if trun_flags & 0x000001 != 0 {
            at += 4; // data_offset: the payload follows the moof, so it is implied
        }
        if trun_flags & 0x000004 != 0 {
            at += 4; // first_sample_flags
        }
        for _ in 0..count {
            let mut duration = default_duration;
            let mut size = default_size;
            if trun_flags & 0x000100 != 0 {
                duration = u32_at(&trun.payload, at)?;
                at += 4;
            }
            if trun_flags & 0x000200 != 0 {
                size = u32_at(&trun.payload, at)?;
                at += 4;
            }
            if trun_flags & 0x000400 != 0 {
                at += 4; // sample_flags
            }
            if trun_flags & 0x000800 != 0 {
                at += 4; // sample_composition_time_offset
            }
            if size == 0 {
                return Err("片段里有一个样本长度为 0".to_owned());
            }
            samples.push(Sample { size, duration });
        }
    }
    Ok(())
}

/// Build `stts` from the sample durations, run-length encoded.
fn build_stts(samples: &[Sample]) -> RawBox {
    let mut entries: Vec<(u32, u32)> = Vec::new();
    for sample in samples {
        match entries.last_mut() {
            Some((count, duration)) if *duration == sample.duration => *count += 1,
            _ => entries.push((1, sample.duration)),
        }
    }
    let mut payload = vec![0u8; 4];
    payload.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for (count, duration) in entries {
        payload.extend_from_slice(&count.to_be_bytes());
        payload.extend_from_slice(&duration.to_be_bytes());
    }
    RawBox {
        kind: *b"stts",
        payload,
    }
}

/// Build `stsz`: every sample's size, listed.
fn build_stsz(samples: &[Sample]) -> RawBox {
    let mut payload = vec![0u8; 4];
    payload.extend_from_slice(&0u32.to_be_bytes()); // no uniform size
    payload.extend_from_slice(&(samples.len() as u32).to_be_bytes());
    for sample in samples {
        payload.extend_from_slice(&sample.size.to_be_bytes());
    }
    RawBox {
        kind: *b"stsz",
        payload,
    }
}

/// Build `stsc`: one chunk holding every sample.
///
/// A single chunk is the simplest table the format allows, and every reader
/// accepts it. It costs one `stco` entry instead of one per fragment.
fn build_stsc() -> RawBox {
    let mut payload = vec![0u8; 4];
    payload.extend_from_slice(&1u32.to_be_bytes()); // one entry
    payload.extend_from_slice(&1u32.to_be_bytes()); // first_chunk
    payload.extend_from_slice(&u32::MAX.to_be_bytes()); // samples_per_chunk: the rest
    payload.extend_from_slice(&1u32.to_be_bytes()); // sample_description_index
    RawBox {
        kind: *b"stsc",
        payload,
    }
}

/// Build `stco`: where the single chunk starts.
fn build_stco(offset: u32) -> RawBox {
    let mut payload = vec![0u8; 4];
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&offset.to_be_bytes());
    RawBox {
        kind: *b"stco",
        payload,
    }
}

/// Assemble the progressive `moov`.
///
/// The original boxes are kept and only the durations are patched, so anything
/// this does not understand (the sample rate table, the language, the AAC decoder
/// configuration) survives untouched. `mvex` is dropped: it exists to promise
/// fragments, and there are none any more.
fn build_moov(init: &Init, samples: &[Sample], chunk_offset: u32) -> Result<RawBox, String> {
    let media_duration: u64 = samples.iter().map(|s| u64::from(s.duration)).sum();
    let movie_duration = if init.media_timescale == 0 {
        media_duration
    } else {
        media_duration * u64::from(init.movie_timescale) / u64::from(init.media_timescale)
    };

    let mut moov_children: Vec<RawBox> = Vec::new();
    for mut child in init.moov.children()? {
        match &child.kind {
            b"mvhd" => {
                // version/flags(4) creation(4|8) modification(4|8) timescale(4)
                let after = if child.payload.first() == Some(&1) {
                    16
                } else {
                    12
                };
                patch_duration(&mut child.payload, after, movie_duration)?;
                moov_children.push(child);
            }
            b"trak" => {
                let mut trak_children: Vec<RawBox> = Vec::new();
                for mut trak_child in child.children()? {
                    match &trak_child.kind {
                        b"tkhd" => {
                            // version/flags(4) creation(4|8) modification(4|8)
                            // track_id(4) reserved(4) timescale lives in mdhd, so
                            // tkhd's duration is already in movie units.
                            let after = if trak_child.payload.first() == Some(&1) {
                                24
                            } else {
                                16
                            };
                            patch_duration(&mut trak_child.payload, after, movie_duration)?;
                            trak_children.push(trak_child);
                        }
                        b"mdia" => {
                            let mut mdia_children: Vec<RawBox> = Vec::new();
                            for mut mdia_child in trak_child.children()? {
                                match &mdia_child.kind {
                                    b"mdhd" => {
                                        let after = if mdia_child.payload.first() == Some(&1) {
                                            16
                                        } else {
                                            12
                                        };
                                        patch_duration(
                                            &mut mdia_child.payload,
                                            after,
                                            media_duration,
                                        )?;
                                        mdia_children.push(mdia_child);
                                    }
                                    b"minf" => {
                                        let mut minf_children: Vec<RawBox> = Vec::new();
                                        for minf_child in mdia_child.children()? {
                                            if minf_child.is(b"stbl") {
                                                // Keep `stsd` (the AAC config) and
                                                // replace the empty tables.
                                                let stsd = minf_child
                                                    .children()?
                                                    .into_iter()
                                                    .find(|box_| box_.is(b"stsd"))
                                                    .unwrap_or_else(|| init.stsd.clone());
                                                let stbl = vec![
                                                    stsd,
                                                    build_stts(samples),
                                                    build_stsc(),
                                                    build_stsz(samples),
                                                    build_stco(chunk_offset),
                                                ];
                                                let mut payload = Vec::new();
                                                for box_ in &stbl {
                                                    box_.serialize(&mut payload);
                                                }
                                                minf_children.push(RawBox {
                                                    kind: *b"stbl",
                                                    payload,
                                                });
                                            } else {
                                                minf_children.push(minf_child);
                                            }
                                        }
                                        mdia_children.push(container(*b"minf", minf_children));
                                    }
                                    _ => mdia_children.push(mdia_child),
                                }
                            }
                            trak_children.push(container(*b"mdia", mdia_children));
                        }
                        _ => trak_children.push(trak_child),
                    }
                }
                moov_children.push(container(*b"trak", trak_children));
            }
            b"mvex" => {}
            _ => moov_children.push(child),
        }
    }

    Ok(container(*b"moov", moov_children))
}

/// The `ftyp` a plain `.m4a` should carry.
///
/// The DASH init segment declares `isom`/`mp41` — video brands for a streaming
/// fragment. Copying them made CoreAudio report the file as `mp4f` rather than an
/// audio file, and that is what decides the icon and the default application, so
/// the brand is rewritten along with the tables.
fn m4a_ftyp() -> RawBox {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"M4A "); // major brand
    payload.extend_from_slice(&0u32.to_be_bytes()); // minor version
    payload.extend_from_slice(b"M4A "); // compatible brands
    payload.extend_from_slice(b"isom");
    payload.extend_from_slice(b"mp42");
    RawBox {
        kind: *b"ftyp",
        payload,
    }
}

fn container(kind: [u8; 4], children: Vec<RawBox>) -> RawBox {
    let mut payload = Vec::new();
    for child in &children {
        child.serialize(&mut payload);
    }
    RawBox { kind, payload }
}

/// Rewrite `segment` as a progressive MP4 at `out`.
///
/// Lossless: the sample payloads are copied byte for byte, only the container
/// changes. The file is read twice — once for the boxes, once to stream the
/// payloads — so a 200 MB segment never has to fit in memory.
pub fn export_m4a(segment: &Path, out: &Path) -> Result<(), String> {
    let input = File::open(segment).map_err(|err| format!("打开缓存文件失败：{err}"))?;
    let end = input
        .metadata()
        .map_err(|err| format!("读取缓存文件大小失败：{err}"))?
        .len();
    let mut reader = BufReader::with_capacity(64 * 1024, input);

    let init = read_init(&mut reader, end)?;
    let samples = read_samples(&mut reader, 0, end, &init)?;

    // Lay the output out first: the chunk offset is the only thing the sample
    // tables need that depends on the sizes of everything before the mdat.
    let mut ftyp_bytes = Vec::new();
    m4a_ftyp().serialize(&mut ftyp_bytes);

    // The moov's size depends on the tables, which depend on the chunk offset,
    // which depends on the moov's size. The tables' length is fixed once the
    // sample count is known, so build it once with a placeholder and measure.
    let placeholder = build_moov(&init, &samples, 0)?;
    let mut moov_bytes = Vec::new();
    placeholder.serialize(&mut moov_bytes);

    let mdat_payload: u64 = samples.iter().map(|s| u64::from(s.size)).sum();
    let chunk_offset = ftyp_bytes.len() as u64 + moov_bytes.len() as u64 + HEADER;
    let chunk_offset = u32::try_from(chunk_offset)
        .map_err(|_| "导出文件过大，超出了 32 位偏移（需要 co64）".to_owned())?;

    let moov = build_moov(&init, &samples, chunk_offset)?;
    let mut moov_bytes = Vec::new();
    moov.serialize(&mut moov_bytes);

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("创建导出目录失败：{err}"))?;
    }
    let file = File::create(out).map_err(|err| format!("创建导出文件失败：{err}"))?;
    let mut writer = BufWriter::with_capacity(256 * 1024, file);
    writer
        .write_all(&ftyp_bytes)
        .and_then(|()| writer.write_all(&moov_bytes))
        .map_err(|err| format!("写入文件头失败：{err}"))?;

    // `mdat` with a 64-bit size if it needs one, so a long compilation still fits.
    let mdat_size = mdat_payload + HEADER;
    if mdat_size > u64::from(u32::MAX) {
        writer
            .write_all(&1u32.to_be_bytes())
            .and_then(|()| writer.write_all(b"mdat"))
            .and_then(|()| writer.write_all(&(mdat_size + 8).to_be_bytes()))
            .map_err(|err| format!("写入 mdat 头失败：{err}"))?;
    } else {
        writer
            .write_all(&(mdat_size as u32).to_be_bytes())
            .and_then(|()| writer.write_all(b"mdat"))
            .map_err(|err| format!("写入 mdat 头失败：{err}"))?;
    }

    copy_sample_payloads(&mut reader, &mut writer, end, &samples, mdat_payload)?;

    writer
        .flush()
        .map_err(|err| format!("写入导出文件失败：{err}"))?;
    Ok(())
}

/// Stream every sample's bytes, in order, from the fragment `mdat`s.
fn copy_sample_payloads<R: Read + Seek, W: Write>(
    reader: &mut R,
    writer: &mut W,
    end: u64,
    samples: &[Sample],
    expected: u64,
) -> Result<(), String> {
    let mut remaining = samples.iter();
    let mut current = remaining.next();
    let mut written = 0u64;
    let mut buffer = vec![0u8; 256 * 1024];

    let mut offset = 0u64;
    while offset + HEADER <= end {
        reader
            .seek(SeekFrom::Start(offset))
            .map_err(|err| format!("定位 mdat 失败：{err}"))?;
        let (kind, len) = read_header(reader, offset)?;
        if &kind == b"mdat" {
            // Walk the samples that live in this mdat, in order. A sample never
            // spans two mdats in Bilibili's segments; if one did, the count check
            // below would catch it.
            let mut available = len;
            while available > 0 {
                let Some(sample) = current else {
                    break;
                };
                if u64::from(sample.size) > available {
                    break;
                }
                let mut left = u64::from(sample.size);
                while left > 0 {
                    let take = left.min(buffer.len() as u64) as usize;
                    reader
                        .read_exact(&mut buffer[..take])
                        .map_err(|err| format!("读取样本数据失败：{err}"))?;
                    writer
                        .write_all(&buffer[..take])
                        .map_err(|err| format!("写入样本数据失败：{err}"))?;
                    left -= take as u64;
                    written += take as u64;
                }
                available -= u64::from(sample.size);
                current = remaining.next();
            }
        }
        offset += len + HEADER;
    }

    if written != expected || current.is_some() {
        return Err(format!(
            "样本数据与索引不一致（写入 {written} 字节，期望 {expected}）"
        ));
    }
    Ok(())
}

/// A file name for an exported track: `歌名 - 作者.m4a`.
///
/// Titles are full of things a file system dislikes (`/` in "24/7", `:` in a
/// time, newlines from a copy-pasted page title), so separators and control
/// characters are replaced, runs of whitespace collapse, and the result is capped
/// so a long title cannot exceed a file name limit. Returns just the file name:
/// the caller decides the directory.
pub fn file_name(title: &str, author: &str) -> String {
    fn clean(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                // Path separators and the Windows-reserved characters, plus the
                // control characters a pasted title can carry.
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('_'),
                ch if ch.is_control() => out.push(' '),
                ch => out.push(ch),
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    let title = clean(title);
    let author = clean(author);
    let stem = match (title.is_empty(), author.is_empty()) {
        (false, false) => format!("{title} - {author}"),
        (false, true) => title,
        (true, false) => author,
        (true, true) => "audio".to_owned(),
    };
    // Cut on a character boundary, not a byte one: a byte slice would panic in
    // the middle of a multi-byte character.
    let mut stem: String = stem.chars().take(120).collect();
    while stem.ends_with([' ', '.']) {
        stem.pop();
    }
    if stem.is_empty() {
        stem = "audio".to_owned();
    }
    format!("{stem}.m4a")
}

/// Write a playable copy of a cached file at `out`.
///
/// A cached `.m4a` is already the file the user wants, so it is copied; a raw
/// `.m4s` fragment is remuxed first. The extension is what tells them apart,
/// because the cache names them by what they are.
pub fn export_any(segment: &Path, out: &Path) -> Result<(), String> {
    let playable = segment
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"));
    if !playable {
        return export_m4a(segment, out);
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("创建导出目录失败：{err}"))?;
    }
    std::fs::copy(segment, out)
        .map(|_| ())
        .map_err(|err| format!("复制到导出目录失败：{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn box_bytes(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&((payload.len() as u32) + 8).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    fn full_box(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
        let mut payload = vec![version, 0, 0, 0];
        payload.extend_from_slice(body);
        box_bytes(kind, &payload)
    }

    /// A minimal fragmented MP4 shaped like Bilibili's: an init segment with an
    /// empty `stbl` plus a `mvex`, then `moof`/`mdat` pairs.
    ///
    /// `sizes` are the per-sample payload lengths; each gets a distinct byte so a
    /// test can prove the payloads were copied rather than rebuilt.
    fn synthetic_fmp4(sizes: &[u32], duration: u32) -> Vec<u8> {
        let mut moov_children = Vec::new();
        // mvhd v0: creation(4) modification(4) timescale(4) duration(4)
        let mut mvhd = Vec::new();
        mvhd.extend_from_slice(&0u32.to_be_bytes());
        mvhd.extend_from_slice(&0u32.to_be_bytes());
        mvhd.extend_from_slice(&1000u32.to_be_bytes());
        mvhd.extend_from_slice(&0u32.to_be_bytes());
        moov_children.push(full_box(b"mvhd", 0, &mvhd));

        // trak > tkhd + mdia > mdhd + hdlr + minf > smhd + dinf + stbl > stsd
        let mut tkhd = Vec::new();
        tkhd.extend_from_slice(&0u32.to_be_bytes()); // creation
        tkhd.extend_from_slice(&0u32.to_be_bytes()); // modification
        tkhd.extend_from_slice(&1u32.to_be_bytes()); // track_id
        tkhd.extend_from_slice(&0u32.to_be_bytes()); // reserved
        tkhd.extend_from_slice(&0u32.to_be_bytes()); // duration
        let tkhd = full_box(b"tkhd", 0, &tkhd);

        let mut mdhd = Vec::new();
        mdhd.extend_from_slice(&0u32.to_be_bytes()); // creation
        mdhd.extend_from_slice(&0u32.to_be_bytes()); // modification
        mdhd.extend_from_slice(&44100u32.to_be_bytes()); // timescale
        mdhd.extend_from_slice(&0u32.to_be_bytes()); // duration
        let mdhd = full_box(b"mdhd", 0, &mdhd);

        let hdlr = full_box(b"hdlr", 0, &[0u8; 20]);
        let smhd = full_box(b"smhd", 0, &[0u8; 4]);
        let dinf = box_bytes(b"dinf", &box_bytes(b"dref", &[0, 0, 0, 0, 0, 0, 0, 1]));
        // A stand-in for the AAC decoder configuration: opaque, must survive.
        let stsd = box_bytes(b"stsd", &[0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 4]);
        let stbl = box_bytes(b"stbl", &stsd);
        let minf = box_bytes(b"minf", &[smhd, dinf, stbl].concat());
        let mdia = box_bytes(b"mdia", &[mdhd, hdlr, minf].concat());
        let trak = box_bytes(b"trak", &[tkhd, mdia].concat());
        moov_children.push(trak);

        // mvex > trex with a default duration
        let mut trex = Vec::new();
        trex.extend_from_slice(&1u32.to_be_bytes()); // track_id
        trex.extend_from_slice(&1u32.to_be_bytes()); // description index
        trex.extend_from_slice(&duration.to_be_bytes()); // default duration
        trex.extend_from_slice(&0u32.to_be_bytes()); // default size
        let mvex = box_bytes(b"mvex", &full_box(b"trex", 0, &trex));
        moov_children.push(mvex);

        let moov = box_bytes(b"moov", &moov_children.concat());
        let ftyp = box_bytes(b"ftyp", b"isom\x00\x00\x02\x00isomiso2mp41");

        let mut out = [ftyp, moov].concat();
        for (index, size) in sizes.iter().enumerate() {
            // tfhd with default_sample_duration, tfdt, trun with per-sample sizes
            let mut tfhd_body = Vec::new();
            tfhd_body.extend_from_slice(&1u32.to_be_bytes()); // track_id
            tfhd_body.extend_from_slice(&duration.to_be_bytes()); // default_sample_duration
            let mut tfhd = full_box(b"tfhd", 0, &tfhd_body);
            // flags: default_sample_duration present (0x8), which is what the field
            // above is for.
            tfhd[8..12].copy_from_slice(&0x000008u32.to_be_bytes());

            let mut tfdt = Vec::new();
            tfdt.extend_from_slice(&0u64.to_be_bytes()); // baseMediaDecodeTime
            let tfdt = full_box(b"tfdt", 1, &tfdt);

            let mut trun_body = Vec::new();
            trun_body.extend_from_slice(&1u32.to_be_bytes()); // sample_count
            trun_body.extend_from_slice(&0u32.to_be_bytes()); // data_offset
            trun_body.extend_from_slice(&size.to_be_bytes()); // one sample's size
            let mut trun = full_box(b"trun", 0, &trun_body);
            // flags: data_offset present (0x1) + sample_size (0x200)
            trun[8..12].copy_from_slice(&0x000201u32.to_be_bytes());

            let traf = box_bytes(b"traf", &[tfhd, tfdt, trun].concat());
            let mfhd = full_box(b"mfhd", 0, &(index as u32 + 1).to_be_bytes());
            let moof = box_bytes(b"moof", &[mfhd, traf].concat());

            let payload: Vec<u8> = std::iter::repeat_n(index as u8 + 1, *size as usize).collect();
            out.extend_from_slice(&box_bytes(b"mdat", &payload));
            // the moof goes before its mdat
            let at = out.len() - (payload.len() + 8);
            out.splice(at..at, moof);
        }
        out
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("listenbli-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// Find a top-level box in a file's bytes.
    fn find_box(bytes: &[u8], kind: &[u8; 4]) -> Option<(usize, usize)> {
        let mut at = 0usize;
        while at + 8 <= bytes.len() {
            let size = u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
                as usize;
            if &bytes[at + 4..at + 8] == kind {
                return Some((at, size));
            }
            if size < 8 {
                return None;
            }
            at += size;
        }
        None
    }

    #[test]
    fn a_fragmented_segment_becomes_a_progressive_file() {
        let sizes = [5u32, 7, 3, 11];
        let source = temp("in.m4s");
        let out = temp("out.m4a");
        std::fs::write(&source, synthetic_fmp4(&sizes, 1024)).unwrap();

        export_m4a(&source, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();

        // No fragments survive: a progressive file has one moov and one mdat.
        assert!(find_box(&bytes, b"moof").is_none(), "moof should be gone");
        assert!(
            bytes.starts_with(&[0, 0, 0, 0x1c, b'f', b't', b'y', b'p', b'M', b'4', b'A', b' ']),
            "the brand should say this is an audio file"
        );
        let (moov_at, moov_size) = find_box(&bytes, b"moov").expect("a moov");
        let (mdat_at, mdat_size) = find_box(&bytes, b"mdat").expect("an mdat");

        // `find_nested` reports offsets inside this slice, so read from it.
        let moov = &bytes[moov_at..moov_at + moov_size];

        // stsz: header(8) + version/flags(4) + uniform size(4) + count(4) + sizes
        let stsz = find_nested(moov, b"stsz").expect("stsz");
        let read_sizes: Vec<u32> = (0..sizes.len())
            .map(|index| {
                let at = stsz + 20 + index * 4;
                u32::from_be_bytes(moov[at..at + 4].try_into().unwrap())
            })
            .collect();
        assert_eq!(read_sizes, sizes, "stsz should list the sample sizes");

        // stco: header(8) + version/flags(4) + entry count(4) + first offset(4)
        let stco = find_nested(moov, b"stco").expect("stco");
        let chunk = u32::from_be_bytes(moov[stco + 16..stco + 20].try_into().unwrap());
        assert_eq!(
            chunk as usize,
            mdat_at + 8,
            "stco should point at the first sample in the mdat"
        );

        // The payloads are copied, not rebuilt: 5×1, 7×2, 3×3, 11×4.
        let payload = &bytes[mdat_at + 8..mdat_at + mdat_size];
        let expected: Vec<u8> = sizes
            .iter()
            .enumerate()
            .flat_map(|(index, size)| std::iter::repeat_n(index as u8 + 1, *size as usize))
            .collect();
        assert_eq!(payload, expected, "the audio bytes must be untouched");

        // Durations are patched from the fragments: four samples of 1024 at
        // 44100 Hz, so 4096 media units and 4096 × 1000 / 44100 movie units.
        // header(8) + version/flags(4) + creation(4) + modification(4) +
        // timescale(4) + duration(4): the duration is the fifth field.
        let mdhd = find_nested(moov, b"mdhd").expect("mdhd");
        let media = u32::from_be_bytes(moov[mdhd + 24..mdhd + 28].try_into().unwrap());
        assert_eq!(media, 4096, "mdhd should total the sample durations");
        let mvhd = find_nested(moov, b"mvhd").expect("mvhd");
        let movie = u32::from_be_bytes(moov[mvhd + 24..mvhd + 28].try_into().unwrap());
        assert_eq!(movie, 4096 * 1000 / 44100, "mvhd should be in movie units");

        // The decoder configuration is carried over verbatim.
        let stsd = find_nested(moov, b"stsd").expect("stsd");
        assert_eq!(
            &moov[stsd..stsd + 20],
            &box_bytes(b"stsd", &[0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 4])
        );

        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&out);
    }

    /// Depth-first search for a box anywhere inside a blob.
    fn find_nested(bytes: &[u8], kind: &[u8; 4]) -> Option<usize> {
        let mut at = 0usize;
        while at + 8 <= bytes.len() {
            let size = u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
                as usize;
            if size < 8 || at + size > bytes.len() {
                return None;
            }
            if &bytes[at + 4..at + 8] == kind {
                return Some(at);
            }
            // Containers worth descending into; leaves are skipped.
            let payload = &bytes[at + 8..at + size];
            if matches!(
                &bytes[at + 4..at + 8],
                b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" | b"mvex"
            ) {
                if let Some(found) = find_nested(payload, kind) {
                    return Some(at + 8 + found);
                }
            }
            at += size;
        }
        None
    }

    /// Every sample rodio's decoder yields, as its raw `f32` stream.
    fn decode(path: &Path) -> Vec<f32> {
        let file = File::open(path).expect("open for decoding");
        let decoder = rodio::Decoder::new(BufReader::new(file)).expect("a decodable file");
        decoder.collect()
    }

    /// The whole point of a remux is that the audio does not change, so the proof
    /// is to decode both files and compare the samples.
    ///
    /// Ignored by default because it needs a real segment. Point
    /// `LISTENBLI_SEGMENT` at any file from the app's audio cache:
    ///
    /// ```text
    /// LISTENBLI_SEGMENT=~/Library/Caches/listenBli/audio/<cid>_30280.m4s \
    ///   cargo test --lib -- --ignored the_exported_file_decodes_to_the_same_audio --nocapture
    /// ```
    #[test]
    #[ignore = "needs a real cached segment in LISTENBLI_SEGMENT"]
    fn the_exported_file_decodes_to_the_same_audio() {
        let source = std::env::var("LISTENBLI_SEGMENT")
            .expect("set LISTENBLI_SEGMENT to a cached .m4s file");
        let source = PathBuf::from(source);
        let out = temp("decoded.m4a");

        export_m4a(&source, &out).expect("the export should succeed");
        let original = decode(&source);
        let exported = decode(&out);

        assert!(!original.is_empty(), "the source decoded to nothing");
        assert_eq!(
            original.len(),
            exported.len(),
            "the export should hold the same number of samples"
        );
        let worst = original
            .iter()
            .zip(&exported)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        eprintln!(
            "compared {} samples from {} (max difference {worst})",
            original.len(),
            source.display()
        );
        assert_eq!(worst, 0.0, "a remux must not alter a single sample");

        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn a_file_name_survives_titles_that_are_not_file_names() {
        assert_eq!(file_name("晴天", "周杰伦"), "晴天 - 周杰伦.m4a");
        // Separators and reserved characters become underscores.
        assert_eq!(file_name("24/7 Live", "A:B"), "24_7 Live - A_B.m4a");
        // Newlines and tabs collapse, and the ends are trimmed.
        assert_eq!(file_name("  多\n行\t标题 ", ""), "多 行 标题.m4a");
        // Nothing usable left: a placeholder rather than a hidden file.
        assert_eq!(file_name("...", ""), "audio.m4a");
        // Long titles are cut without splitting a character.
        let long = file_name(&"字".repeat(400), "作者");
        assert!(
            long.chars().count() <= 126,
            "got {} chars",
            long.chars().count()
        );
        assert!(long.ends_with(".m4a"));
    }

    #[test]
    fn an_already_playable_file_is_copied_rather_than_remuxed() {
        // A progressive file has no `moof`, so remuxing it would fail; the copy
        // path is what makes exporting a converted cache work.
        let source = temp("ready.m4a");
        let out = temp("ready-out.m4a");
        let bytes = b"not really an m4a, but the extension says copy me".to_vec();
        std::fs::write(&source, &bytes).unwrap();
        std::fs::remove_file(&out).ok();

        export_any(&source, &out).expect("a copy");
        assert_eq!(std::fs::read(&out).unwrap(), bytes);

        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn a_file_that_is_not_a_segment_is_refused() {
        let source = temp("garbage.m4s");
        let out = temp("garbage.m4a");
        std::fs::write(&source, b"<html>403 forbidden</html>").unwrap();
        let err = export_m4a(&source, &out).unwrap_err();
        assert!(err.contains("ftyp") || err.contains("moov"), "got {err}");
        assert!(
            !out.exists(),
            "a refusal must not leave a half-written file"
        );
    }

    #[test]
    fn the_duration_field_moves_with_the_box_version() {
        // v0: version/flags + creation(4) + modification(4) + timescale(4) + duration
        let mut v0 = vec![0u8; 20];
        patch_duration(&mut v0, 12, 1234).unwrap();
        assert_eq!(u32_at(&v0, 16).unwrap(), 1234);

        // v1 widens both times, so the same duration sits eight bytes later.
        let mut v1 = vec![0u8; 32];
        v1[0] = 1;
        patch_duration(&mut v1, 16, 5_000_000_000).unwrap();
        assert_eq!(u64_at(&v1, 24).unwrap(), 5_000_000_000);
    }
}

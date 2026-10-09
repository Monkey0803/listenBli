//! M0 de-risking spike: prove that we can fetch a Bilibili DASH audio segment
//! and decode it with the pure-Rust stack (rodio -> symphonia isomp4 + AAC).
//!
//! Run with:
//! ```text
//! cargo run --example spike_decode
//! ```
//!
//! It performs no playback, so it can be run before any GUI exists. Success is
//! judged by the printed sample rate / channel count / decoded duration, and by
//! a non-silent RMS measurement. A WAV of the first few seconds is written to the
//! temp directory for a human to actually listen to.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;

use rodio::{Decoder, Source};

const BVID: &str = "BV1GJ411x7h7";
const CID: i64 = 137649199;
const EXPECTED_STREAM_ID: u32 = 30280; // 192K AAC-LC

#[cfg(target_os = "macos")]
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
#[cfg(target_os = "windows")]
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const REFERER: &str = "https://www.bilibili.com/";

fn main() -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(120))
        .build()?;

    let cache: PathBuf = std::env::temp_dir().join(format!("listenbli-spike-{CID}.m4s"));

    let bytes = if cache.is_file() {
        println!("[1/4] reusing cached segment at {}", cache.display());
        std::fs::read(&cache)?
    } else {
        println!("[1/4] requesting playurl for {BVID}/{CID}");
        let playurl = format!(
            "https://api.bilibili.com/x/player/playurl?bvid={BVID}&cid={CID}&fnval=4048&fnver=0&fourk=1"
        );
        let json: serde_json::Value = client
            .get(&playurl)
            .header("Referer", REFERER)
            .send()?
            .json()?;

        let audio = json["data"]["dash"]["audio"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("no dash.audio in response: {json}"))?;
        println!("       available audio streams:");
        for a in audio {
            println!(
                "         id={} codecs={} bandwidth={}",
                a["id"], a["codecs"], a["bandwidth"]
            );
        }

        let chosen = audio
            .iter()
            .find(|a| a["id"].as_u64() == Some(EXPECTED_STREAM_ID as u64))
            .ok_or_else(|| anyhow::anyhow!("stream {EXPECTED_STREAM_ID} not offered"))?;
        let url = chosen["baseUrl"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("stream {EXPECTED_STREAM_ID} has no baseUrl"))?;

        println!("[2/4] downloading stream {EXPECTED_STREAM_ID}");
        let mut body = client.get(url).header("Referer", REFERER).send()?;
        let status = body.status();
        let mut buf = Vec::new();
        std::io::copy(&mut body, &mut buf)?;
        println!("       HTTP {status}, {} bytes", buf.len());

        anyhow::ensure!(
            buf.len() > 8 && &buf[4..8] == b"ftyp",
            "downloaded payload is not an ISO-BMFF file (first bytes: {:02x?})",
            &buf[..8.min(buf.len())]
        );
        std::fs::write(&cache, &buf)?;
        buf
    };

    // Summarise the container so the fMP4 assumption is visible, not assumed.
    println!("[3/4] container boxes present:");
    for marker in [
        b"ftyp".as_slice(),
        b"moov",
        b"mvex",
        b"trex",
        b"stsd",
        b"esds",
        b"sidx",
        b"moof",
        b"traf",
        b"trun",
        b"mdat",
    ] {
        let found = bytes.windows(marker.len()).any(|w| w == marker);
        println!(
            "       {} {}",
            marker.iter().map(|b| *b as char).collect::<String>(),
            if found { "yes" } else { "NO" }
        );
    }

    println!("[4/4] decoding with symphonia");
    let mut cursor = Cursor::new(bytes.clone());
    cursor.seek(SeekFrom::Start(0))?;
    let decoder = Decoder::try_from(cursor)?;

    let sample_rate = decoder.sample_rate().get();
    let channels = decoder.channels().get();
    let total = decoder.total_duration();
    println!("       sample_rate = {sample_rate} Hz");
    println!("       channels    = {channels}");
    println!("       total       = {total:?}");

    // Decode the whole thing to measure real energy and duration.
    let mut decoded_frames: u64 = 0;
    let mut sum_squares: f64 = 0.0;
    let mut peak: f32 = 0.0;
    let mut non_finite = 0u64;
    for sample in decoder {
        if !sample.is_finite() {
            non_finite += 1;
        } else {
            sum_squares += (sample as f64) * (sample as f64);
            peak = peak.max(sample.abs());
        }
        decoded_frames += 1;
    }

    let decoded_samples_per_channel = decoded_frames / channels.max(1) as u64;
    let decoded_seconds = decoded_samples_per_channel as f64 / sample_rate as f64;
    let rms = if decoded_frames > 0 {
        (sum_squares / decoded_frames as f64).sqrt()
    } else {
        0.0
    };

    println!("       decoded frames        = {decoded_frames}");
    println!("       decoded duration      = {decoded_seconds:.2} s");
    println!("       RMS                   = {rms:.5}");
    println!("       peak                  = {peak:.5}");
    println!("       non-finite samples    = {non_finite}");

    // Write the first 8 seconds as a WAV so a human can confirm it is music.
    let wav_path = std::env::temp_dir().join("listenbli-spike-preview.wav");
    write_wav_preview(&bytes, sample_rate, channels, 8, &wav_path)?;
    println!("       preview written to    = {}", wav_path.display());

    // ---- assertions -------------------------------------------------------
    anyhow::ensure!(sample_rate > 0, "sample rate was zero");
    anyhow::ensure!(channels > 0, "channel count was zero");
    anyhow::ensure!(non_finite == 0, "decoder produced non-finite samples");
    anyhow::ensure!(
        decoded_seconds > 60.0,
        "decoded only {decoded_seconds:.2}s, expected a full song"
    );
    anyhow::ensure!(
        rms > 0.001,
        "decoded audio looks silent (RMS {rms:.6}) - decode likely failed"
    );

    if let Some(expected) = total {
        let delta = (expected.as_secs_f64() - decoded_seconds).abs();
        println!("       duration delta        = {delta:.2} s");
        anyhow::ensure!(
            delta < 5.0,
            "decoded duration differs from reported duration by {delta:.2}s"
        );
    }

    println!("\n[5/5] seeking (the UI needs this for the progress bar and lyric clicks)");
    // NOTE: `total_duration()` is `None` for these DASH fMP4 segments, so the
    // player must be told the duration by the API layer instead.
    anyhow::ensure!(
        total.is_none(),
        "expected total_duration to be None for fMP4"
    );
    println!("       total_duration() = None as expected -> duration must come from the API");

    let mut seekable = Decoder::try_from(Cursor::new(bytes.clone()))?;
    match seekable.try_seek(Duration::from_secs(60)) {
        Ok(()) => {
            // Read 1 second after the seek and confirm it is still audible.
            let want = sample_rate as usize * channels as usize;
            let mut energy = 0.0f64;
            let mut count = 0usize;
            for sample in seekable.take(want) {
                energy += (sample as f64) * (sample as f64);
                count += 1;
            }
            let rms = if count > 0 {
                (energy / count as f64).sqrt()
            } else {
                0.0
            };
            println!("       seek to 60s ok, RMS after seek = {rms:.5} ({count} samples)");
            anyhow::ensure!(count > 0, "no samples after seek");
            anyhow::ensure!(rms > 0.001, "audio after seek looked silent (RMS {rms:.6})");
        }
        Err(err) => {
            anyhow::bail!("try_seek failed: {err:?}");
        }
    }

    println!("\nSPIKE PASSED: symphonia decodes Bilibili DASH fMP4/AAC-LC.");
    Ok(())
}

/// Minimal 16-bit PCM WAV writer (avoids enabling rodio's `wav_output` feature).
fn write_wav_preview(
    bytes: &[u8],
    sample_rate: u32,
    channels: u16,
    seconds: u32,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    let decoder = Decoder::try_from(Cursor::new(bytes.to_vec()))?;
    let want = sample_rate as u64 * channels as u64 * seconds as u64;

    let mut pcm: Vec<i16> = Vec::new();
    for sample in decoder.take(want as usize) {
        let clamped = sample.clamp(-1.0, 1.0);
        pcm.push((clamped * i16::MAX as f32) as i16);
    }

    let data_len = (pcm.len() * 2) as u32;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels.saturating_mul(2);

    let mut out = Vec::with_capacity(44 + pcm.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in &pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }

    let mut file = std::fs::File::create(path)?;
    file.write_all(&out)?;
    Ok(())
}

// Keep `Read`/`Write`/`Seek` imports honest even if unused on some platform.
#[allow(dead_code)]
fn _assert_traits_in_scope<R: Read, W: Write, S: Seek>() {}

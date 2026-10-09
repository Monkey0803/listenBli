//! Experiment: what is the minimum prefix of a Bilibili DASH audio segment that
//! can be decoded, and does decoding survive the file growing underneath it?
//!
//! The container layout makes streaming *look* cheap: the init segment
//! (`ftyp` + `moov` + `sidx`) is only ~1.5 KB and the `moof`/`mdat` fragments
//! that follow are sequential.
//!
//! ```text
//! cargo run --example spike_streaming -- /tmp/t.m4s
//! ```

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use rodio::{Decoder, Source};

fn main() -> anyhow::Result<()> {
    let path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("listenbli-spike-137649199.m4s"));
    let full = std::fs::read(&path)?;
    println!("source: {} ({} bytes)\n", path.display(), full.len());

    let boxes = top_level_boxes(&full);
    let init_end = boxes
        .iter()
        .filter(|(kind, _, _)| kind == "sidx")
        .map(|(_, _, end)| *end)
        .next()
        .ok_or_else(|| anyhow::anyhow!("no sidx box found"))?;
    let moof_offsets: Vec<usize> = boxes
        .iter()
        .filter(|(kind, _, _)| kind == "moof")
        .map(|(_, start, _)| *start)
        .collect();

    println!("init segment ends at byte {init_end}");
    println!("{} moof/mdat fragments follow\n", moof_offsets.len());

    println!("{:<34} {:>10} {:>14}", "prefix", "bytes", "decoded");
    println!("{}", "-".repeat(60));

    let mut prefixes: Vec<(String, usize)> = vec![
        (
            "init only (mid-sidx cut)".into(),
            init_end.saturating_sub(200).max(1),
        ),
        ("init only (exact)".into(), init_end),
        ("init + half a fragment".into(), init_end + 5000),
    ];
    for n in [1usize, 2, 5, 20, 60] {
        if let Some(offset) = moof_offsets.get(n) {
            prefixes.push((format!("init + {n} fragments (boundary)"), *offset));
        }
    }
    prefixes.push(("full file".into(), full.len()));

    let mut budget: Option<(String, usize, f64)> = None;
    for (label, size) in prefixes {
        let size = size.min(full.len());
        let tmp = std::env::temp_dir().join("listenbli-prefix.m4s");
        std::fs::write(&tmp, &full[..size])?;

        let decoded = match Decoder::try_from(File::open(&tmp)?) {
            Ok(decoder) => {
                let rate = decoder.sample_rate().get() as f64;
                let ch = decoder.channels().get() as f64;
                let mut frames = 0u64;
                for _ in decoder {
                    frames += 1;
                }
                frames as f64 / ch / rate
            }
            Err(_) => f64::NAN,
        };

        if decoded.is_nan() {
            println!("{label:<34} {size:>10} {:>14}", "open FAILED");
        } else {
            println!("{label:<34} {size:>10} {:>13.1}s", decoded);
            if budget.is_none() && decoded >= 10.0 {
                budget = Some((label, size, decoded));
            }
        }
        let _ = std::fs::remove_file(&tmp);
    }

    if let Some((label, size, seconds)) = &budget {
        println!("\nminimum useful prefix: {label} -> {size} bytes gives {seconds:.1}s of audio");
    }

    println!("\n--- append-and-continue test ---");
    let (start_prefix, prefix_seconds) = budget
        .as_ref()
        .map(|(_, size, seconds)| (*size, *seconds))
        .unwrap_or((init_end, 0.0));
    let tmp = std::env::temp_dir().join("listenbli-append.m4s");
    std::fs::write(&tmp, &full[..start_prefix])?;

    let mut decoder = match Decoder::try_from(File::open(&tmp)?) {
        Ok(decoder) => decoder,
        Err(err) => {
            println!("decoder could not open the prefix at all: {err}");
            let _ = std::fs::remove_file(&tmp);
            return Ok(());
        }
    };
    let rate = decoder.sample_rate().get() as f64;
    let ch = decoder.channels().get() as f64;

    let mut frames = 0u64;
    let mut appended = false;

    for _sample in decoder.by_ref() {
        frames += 1;
        if !appended && frames as f64 / ch / rate > 3.0 {
            appended = true;
            let mut file = OpenOptions::new().append(true).open(&tmp)?;
            file.write_all(&full[start_prefix..])?;
            file.sync_all()?;
            println!("appended the remaining {} bytes", full.len() - start_prefix);
        }
        if frames as f64 / ch / rate > 900.0 {
            break;
        }
    }

    let decoded_seconds = frames as f64 / ch / rate;
    println!("prefix alone decodes to   {prefix_seconds:.1}s");
    println!("after appending, decoded  {decoded_seconds:.1}s");

    // The meaningful test: did decoding get *past* the prefix?
    if decoded_seconds > prefix_seconds + 2.0 {
        println!(
            "\nRESULT: decoding CONTINUED past the initial end-of-file.\n\
             -> download-into-a-file-and-append is viable."
        );
    } else {
        println!(
            "\nRESULT: decoding STOPPED at the initial end-of-file\n\
             (appending {} more bytes bought nothing).\n\
             -> rodio's Decoder fixes the stream length when it opens, so a\n\
                partially written file is terminal. Streaming therefore needs\n\
                either a custom MediaSource over a blocking buffer, or a\n\
                decoder re-open once more data has landed.",
            full.len() - start_prefix
        );
    }

    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

/// `(fourcc, start, end)` for the top-level boxes of an ISO-BMFF file.
fn top_level_boxes(data: &[u8]) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset + 8 <= data.len() {
        let size = match data[offset..offset + 4].try_into() {
            Ok(v) => u32::from_be_bytes(v) as usize,
            Err(_) => break,
        };
        let kind = String::from_utf8_lossy(&data[offset + 4..offset + 8]).to_string();
        if size < 8 {
            break;
        }
        let end = (offset + size).min(data.len());
        out.push((kind, offset, end));
        offset += size;
    }
    out
}

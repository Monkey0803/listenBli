//! LRC parsing and lyric timeline lookup.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::api::models::SubtitleLine;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    pub time: Duration,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LyricsSource {
    BilibiliSubtitle,
    Netease,
    None,
}

impl LyricsSource {
    pub fn label(self) -> &'static str {
        match self {
            LyricsSource::BilibiliSubtitle => "B站字幕",
            LyricsSource::Netease => "网易云",
            LyricsSource::None => "无",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    pub source: LyricsSource,
}

impl Lyrics {
    pub fn empty(source: LyricsSource) -> Self {
        Self {
            lines: Vec::new(),
            source,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Index of the line that should be highlighted at `position`.
    pub fn current_index(&self, position: Duration) -> Option<usize> {
        current_line_index(&self.lines, position)
    }
}

/// Index of the last line whose timestamp is `<= position`.
///
/// Returns `None` before the first line starts (e.g. during an intro).
pub fn current_line_index(lines: &[LyricLine], position: Duration) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    // Lines are sorted by construction; `partition_point` finds the first line
    // strictly after `position`.
    let after = lines.partition_point(|line| line.time <= position);
    if after == 0 {
        None
    } else {
        Some(after - 1)
    }
}

/// Parse an LRC document into `(time, text)` pairs, applying any `[offset:]` tag.
pub fn parse_lrc(input: &str) -> Vec<(Duration, String)> {
    let mut offset_ms: i64 = 0;
    let mut raw: Vec<(i64, String)> = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut rest = line;
        let mut stamps: Vec<i64> = Vec::new();

        // Consume every leading `[...]` group.
        while let Some(open) = rest.find('[') {
            if open != 0 {
                break;
            }
            let Some(close) = rest.find(']') else { break };
            let tag = &rest[1..close];
            rest = rest[close + 1..].trim_start();

            if let Some(value) = tag.strip_prefix("offset:") {
                offset_ms = value.trim().parse::<i64>().unwrap_or(0);
                continue;
            }
            if let Some(ms) = parse_timestamp(tag) {
                stamps.push(ms);
            }
            // Anything else (`[ti:`, `[ar:`, `[by:`, ...) is metadata: ignore.
        }

        let text = rest.trim();
        if text.is_empty() {
            continue;
        }
        for stamp in stamps {
            raw.push((stamp, text.to_string()));
        }
    }

    for (stamp, _) in raw.iter_mut() {
        *stamp += offset_ms;
    }
    raw.retain(|(stamp, _)| *stamp >= 0);
    raw.sort_by_key(|(stamp, _)| *stamp);

    raw.into_iter()
        .map(|(ms, text)| (Duration::from_millis(ms as u64), text))
        .collect()
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss.xxx` (also tolerates `hh:mm:ss.xx`).
/// Returns milliseconds.
fn parse_timestamp(value: &str) -> Option<i64> {
    let value = value.trim();
    let parts: Vec<&str> = value.split(':').collect();
    let (hours, minutes, seconds) = match parts.len() {
        2 => (0i64, parts[0], parts[1]),
        3 => (parts[0].parse::<i64>().ok()?, parts[1], parts[2]),
        _ => return None,
    };
    let minutes: i64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let total = hours as f64 * 3600.0 + minutes as f64 * 60.0 + seconds;
    Some((total * 1000.0).round() as i64)
}

/// Attach translated lines to their original counterparts.
///
/// NetEase's `tlyric` uses the same timestamps as `lrc`; we allow a small
/// tolerance because the encoders sometimes round differently.
pub fn merge_translation(
    main: Vec<(Duration, String)>,
    translation: Vec<(Duration, String)>,
) -> Vec<LyricLine> {
    const TOLERANCE_MS: u64 = 100;

    main.into_iter()
        .map(|(time, text)| {
            let matched = translation
                .iter()
                .find(|(t, _)| (*t).abs_diff(time).as_millis() as u64 <= TOLERANCE_MS)
                .map(|(_, text)| text.clone())
                // A translation identical to the original adds no value.
                .filter(|t| t != &text);
            LyricLine {
                time,
                text,
                translation: matched,
            }
        })
        .collect()
}

/// Convert a Bilibili CC subtitle body into lyric lines.
pub fn from_subtitle(body: &[SubtitleLine]) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = body
        .iter()
        .filter(|line| !line.content.trim().is_empty())
        .map(|line| LyricLine {
            time: Duration::from_secs_f64(line.from.max(0.0)),
            // Subtitles wrap; collapse the newlines for a single lyric row.
            text: line
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            translation: None,
        })
        .collect();
    lines.sort_by_key(|line| line.time);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lrc() {
        let lines =
            parse_lrc("[00:18.684]We're no strangers to love\n[00:22.657]You know the rules");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, Duration::from_millis(18_684));
        assert_eq!(lines[0].1, "We're no strangers to love");
        assert_eq!(lines[1].0, Duration::from_millis(22_657));
    }

    #[test]
    fn parses_variable_precision_and_missing_fraction() {
        let lines = parse_lrc("[00:01]a\n[00:02.5]b\n[00:03.25]c\n[00:04.125]d");
        let ms: Vec<u64> = lines.iter().map(|(t, _)| t.as_millis() as u64).collect();
        assert_eq!(ms, vec![1000, 2500, 3250, 4125]);
    }

    #[test]
    fn supports_multiple_timestamps_on_one_line() {
        let lines = parse_lrc("[00:10.00][01:20.00]repeated chorus");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].1, "repeated chorus");
        assert_eq!(lines[1].1, "repeated chorus");
        assert_eq!(lines[1].0, Duration::from_secs(80));
    }

    #[test]
    fn applies_offset_tag() {
        let lines = parse_lrc("[offset:+500]\n[00:10.00]shifted later");
        assert_eq!(lines[0].0, Duration::from_millis(10_500));

        let lines = parse_lrc("[offset:-2000]\n[00:10.00]shifted earlier");
        assert_eq!(lines[0].0, Duration::from_millis(8_000));
    }

    #[test]
    fn drops_lines_pushed_before_zero_by_offset() {
        let lines = parse_lrc("[offset:-5000]\n[00:02.00]gone\n[00:10.00]kept");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1, "kept");
    }

    #[test]
    fn ignores_metadata_tags_and_blank_lines() {
        let lrc = "[ti:Never Gonna Give You Up]\n[ar:Rick Astley]\n[by:someone]\n\n[00:01.00]first";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1, "first");
    }

    #[test]
    fn output_is_sorted_even_when_input_is_not() {
        let lines = parse_lrc("[00:30.00]third\n[00:10.00]first\n[00:20.00]second");
        let texts: Vec<&str> = lines.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(texts, vec!["first", "second", "third"]);
    }

    #[test]
    fn tolerates_malformed_input_without_panicking() {
        let lines = parse_lrc("garbage\n[not a time]also garbage\n[00:05.00]ok\n[00:xx]bad");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1, "ok");
        assert!(parse_lrc("").is_empty());
    }

    #[test]
    fn merges_translation_by_timestamp() {
        let main = parse_lrc("[00:10.00]Hello\n[00:20.00]World");
        let trans = parse_lrc("[00:10.00]你好\n[00:20.05]世界");
        let merged = merge_translation(main, trans);
        assert_eq!(merged[0].translation.as_deref(), Some("你好"));
        // 50 ms apart is within tolerance.
        assert_eq!(merged[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn translation_outside_tolerance_is_dropped() {
        let main = parse_lrc("[00:10.00]Hello");
        let trans = parse_lrc("[00:10.50]你好");
        let merged = merge_translation(main, trans);
        assert_eq!(merged[0].translation, None);
    }

    #[test]
    fn identical_translation_is_not_duplicated() {
        let main = parse_lrc("[00:10.00]Hello");
        let trans = parse_lrc("[00:10.00]Hello");
        let merged = merge_translation(main, trans);
        assert_eq!(merged[0].translation, None);
    }

    #[test]
    fn finds_current_line() {
        let lines = merge_translation(parse_lrc("[00:10.00]a\n[00:20.00]b\n[00:30.00]c"), vec![]);
        // Before the first line there is nothing to highlight.
        assert_eq!(current_line_index(&lines, Duration::from_secs(5)), None);
        assert_eq!(current_line_index(&lines, Duration::from_secs(10)), Some(0));
        assert_eq!(current_line_index(&lines, Duration::from_secs(19)), Some(0));
        assert_eq!(current_line_index(&lines, Duration::from_secs(20)), Some(1));
        assert_eq!(
            current_line_index(&lines, Duration::from_secs(999)),
            Some(2)
        );
        assert_eq!(current_line_index(&[], Duration::from_secs(1)), None);
    }

    #[test]
    fn converts_subtitle_body() {
        let body = vec![
            SubtitleLine {
                from: 5.5,
                to: 8.0,
                content: "第一行".into(),
            },
            SubtitleLine {
                from: 1.0,
                to: 4.0,
                content: "   ".into(),
            },
            SubtitleLine {
                from: 2.0,
                to: 4.0,
                content: "line\nwrapped".into(),
            },
        ];
        let lines = from_subtitle(&body);
        assert_eq!(lines.len(), 2, "blank subtitle entries should be skipped");
        assert_eq!(lines[0].time, Duration::from_secs(2));
        assert_eq!(lines[0].text, "line wrapped");
        assert_eq!(lines[1].time, Duration::from_millis(5_500));
    }
}

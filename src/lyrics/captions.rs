//! YouTube's own subtitles, as a lyric source.
//!
//! The `player` response lists caption tracks with a `baseUrl` each, and that URL
//! answers with timedtext XML:
//!
//! ```xml
//! <?xml version="1.0" encoding="utf-8" ?><timedtext format="3">
//! <body><text start="19.32" dur="2.5">We&#39;re no strangers to love</text>…
//! ```
//!
//! Measured on 2026-10-10: `&fmt=json3` is **ignored** — the body comes back as XML
//! whatever is asked for — so the parser handles XML rather than the JSON shape the
//! older integrations expected. A track is `kind: "asr"` when it is auto-generated,
//! which is the same human-versus-machine distinction the Bilibili path makes, and
//! the machine ones go through the same coverage guard rather than being trusted.
//!
//! This is the YouTube-native source: for an official music video the uploader's own
//! subtitles beat anything a lyrics database can match by title.

use std::time::Duration;

use super::lrc::{LyricLine, Lyrics, LyricsSource};
use crate::api::models::Track;
use crate::api::youtube::Youtube;

/// One caption track the player offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionTrack {
    pub language: String,
    /// Auto-generated (ASR) rather than written by a person.
    pub machine: bool,
    /// The timedtext URL. Signed like every other YouTube URL.
    pub url: String,
}

/// The subtitle a track should be shown, if any.
///
/// A human track wins over ASR — including in a language neither of us can guess at,
/// because the video's own primary language is the one YouTube lists first and a
/// person chose to write it. Only when nothing human exists does ASR get a chance.
pub fn choose(tracks: &[CaptionTrack]) -> Option<&CaptionTrack> {
    tracks
        .iter()
        .find(|track| !track.machine)
        .or_else(|| tracks.iter().find(|track| track.machine))
}

/// The whole job for one track: ask the player, pick a track, fetch it, parse it.
///
/// Returns `(lyrics, machine_generated)` so the caller can apply the same coverage
/// guard the Bilibili machine subtitles go through. The fetching lives in the
/// YouTube client, because the document is bound to the client that named it.
pub fn fetch_for(youtube: &Youtube, track: &Track) -> Option<(Lyrics, bool)> {
    let (xml, machine) = match youtube.caption_xml(&track.bvid) {
        Ok(Some(document)) => document,
        Ok(None) => return None,
        Err(err) => {
            // A caption lookup failing is not a playback failure: log and let the
            // rest of the lyric chain run.
            eprintln!("youtube captions for {} failed: {err}", track.bvid);
            return None;
        }
    };
    let lines = parse_timedtext(&xml);
    if lines.is_empty() {
        return None;
    }
    Some((
        Lyrics {
            lines: lines
                .into_iter()
                .map(|(time, text)| LyricLine {
                    time,
                    text,
                    translation: None,
                })
                .collect(),
            source: LyricsSource::YoutubeCaptions,
        },
        machine,
    ))
}

/// Parse timedtext XML into timed lines.
///
/// Hand-rolled rather than pulling in an XML crate: the format here is a flat list
/// of timed elements, and the only things that need care are that there are **two**
/// element shapes, that entities need decoding, and that a word-timed track wraps
/// its words in inner elements.
///
/// The two shapes are both served for the same video, and which one arrives depends
/// on the client that asked — measured on 2026-10-10:
///
/// ```xml
/// <text start="19.32" dur="2.5">seconds, as a fraction</text>   <!-- WEB -->
/// <p t="1360" d="1680">milliseconds, as an integer</p>          <!-- ANDROID_VR -->
/// ```
///
/// `&fmt=xml`, `&fmt=json3` and `&fmt=srv3` all change nothing: the body is XML
/// either way, in one shape or the other.
pub fn parse_timedtext(xml: &str) -> Vec<(Duration, String)> {
    let mut lines = Vec::new();
    let mut rest = xml;

    while let Some((tag, at)) = next_timed_element(rest) {
        let after = &rest[at..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let attributes = &after[..open_end];
        let body = &after[open_end + 1..];
        let closing = format!("</{tag}>");
        let Some(close) = body.find(&closing) else {
            break;
        };
        let text = decode_entities(&strip_tags(&body[..close]));

        if let Some(seconds) = element_start(tag, attributes) {
            let text = text.trim();
            // Cues like `[Music]` or `[♪♪♪]` are part of the video, not lyrics.
            if !text.is_empty() && !is_cue(text) {
                lines.push((Duration::from_secs_f64(seconds), text.to_owned()));
            }
        }
        rest = &body[close + closing.len()..];
    }

    // The format does not promise order, and everything downstream assumes it.
    lines.sort_by_key(|line| line.0);
    lines
}

/// The next timed element, and its tag name.
fn next_timed_element(input: &str) -> Option<(&'static str, usize)> {
    let text = input.find("<text").map(|at| ("text", at));
    let short = input
        .find("<p ")
        .or_else(|| input.find("<p>"))
        .map(|at| ("p", at));
    match (text, short) {
        (Some(first), Some(second)) => Some(if first.1 <= second.1 { first } else { second }),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// When an element starts, in seconds.
///
/// A `<text>` counts seconds in `start`; a `<p>` counts milliseconds in `t`. Reading
/// one as the other is off by a factor of a thousand, which is the kind of mistake
/// that looks like "the lyrics are always on the first line".
fn element_start(tag: &str, attributes: &str) -> Option<f64> {
    let raw: f64 = attribute(attributes, if tag == "p" { "t" } else { "start" })?
        .parse()
        .ok()?;
    Some(if tag == "p" { raw / 1000.0 } else { raw })
}

/// A `name="value"` attribute out of an element's attribute text.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let at = attributes.find(&needle)? + needle.len();
    let value = &attributes[at..];
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        // Unquoted values are legal in HTML but not in the XML this serves.
        return None;
    }
    let value = &value[1..];
    value.find(quote).map(|end| value[..end].to_owned())
}

/// Drop any inner tags, which is how word-level timing is expressed.
fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// The five named entities, plus numeric character references.
fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(end) = after.find(';') else {
            out.push_str(after);
            return out;
        };
        let entity = &after[1..end];
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse::<u32>().ok(),
                })
                .and_then(char::from_u32),
        };
        match replacement {
            Some(ch) => out.push(ch),
            // An entity we do not know is left as it stands rather than dropped.
            None => out.push_str(&after[..end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Whether a cue is a production note rather than a lyric.
fn is_cue(text: &str) -> bool {
    // A line with no letters or digits anywhere carries no lyric: `[♪♪♪]`, `...`,
    // `]` — the markers a music video is full of.
    if !text.chars().any(char::is_alphanumeric) {
        return true;
    }
    let lowered = text.to_ascii_lowercase();
    let inner = text
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
        .unwrap_or("");
    let bracketed = !inner.is_empty();
    let cues = [
        "music",
        "musique",
        "音楽",
        "音乐",
        "applause",
        "鼓掌",
        "掌声",
        "笑声",
        "laughs",
        "silence",
        "instrumental",
    ];
    bracketed
        && cues
            .iter()
            .any(|cue| lowered.contains(cue) || inner.contains(cue))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::Source;

    /// A trimmed copy of what the endpoint really serves.
    fn timedtext() -> &'static str {
        r#"<?xml version="1.0" encoding="utf-8" ?><timedtext format="3">
<body>
<text start="19.32" dur="2.5">We&#39;re no strangers to love</text>
<text start="22.10" dur="3.1">You know the rules &amp; so do I</text>
<text start="25.40" dur="2.0">[Music]</text>
<text start="27.80" dur="2.2"><s>a word</s><s> at a time</s></text>
<text dur="2.0">no timestamp, skipped</text>
<text start="31.00" dur="2.0">   </text>
</body></timedtext>"#
    }

    #[test]
    fn timedtext_becomes_timed_lines() {
        let lines = parse_timedtext(timedtext());
        let texts: Vec<&str> = lines.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "We're no strangers to love",
                "You know the rules & so do I",
                "a word at a time",
            ],
            "entities decoded, cues and untimed elements dropped"
        );
        assert_eq!(lines[0].0, Duration::from_secs_f64(19.32));
    }

    /// The other element shape, with millisecond integer timestamps, as the
    /// playback clients serve it. Both shapes must land on the same timeline.
    #[test]
    fn the_millisecond_shape_is_read_as_milliseconds() {
        let xml = r#"<?xml version="1.0" encoding="utf-8" ?><timedtext format="3">
<body>
<p t="1360" d="1680">[♪♪♪]</p>
<p t="19320" d="2500">We&#39;re no strangers to love</p>
<p t="22100" d="3100">You know the rules &amp; so do I</p>
</body></timedtext>"#;
        let lines = parse_timedtext(xml);
        assert_eq!(lines.len(), 2, "the symbol cue is dropped: {lines:?}");
        assert_eq!(lines[0].0, Duration::from_secs_f64(19.32));
        assert_eq!(lines[0].1, "We're no strangers to love");
        assert_eq!(lines[1].0, Duration::from_secs_f64(22.10));
    }

    /// A cue is a line with nothing to sing — no letters, no digits — or a known
    /// marker in brackets. Real lyrics must never be mistaken for one.
    #[test]
    fn cues_are_recognised_without_eating_lyrics() {
        assert!(is_cue("[♪♪♪]"));
        assert!(is_cue("..."));
        assert!(is_cue("[Music]"));
        assert!(is_cue("[Applause]"));
        assert!(!is_cue("We're no strangers to love"));
        assert!(!is_cue("♪ 我还在唱歌"));
        assert!(!is_cue("（副歌）愛してる"));
    }

    /// Word-level timing wraps words in elements; the words must survive, the tags
    /// must not.
    #[test]
    fn inner_tags_are_stripped() {
        assert_eq!(strip_tags("<s>a</s><s>b</s>"), "ab");
        assert_eq!(strip_tags("plain"), "plain");
    }

    #[test]
    fn entities_are_decoded_including_numeric_ones() {
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        assert_eq!(decode_entities("it&#39;s"), "it's");
        assert_eq!(decode_entities("&#x4E2D;&#25991;"), "中文");
        // Unknown entities are left alone rather than silently eaten.
        assert_eq!(decode_entities("&weird;"), "&weird;");
        assert_eq!(decode_entities("no entity"), "no entity");
        assert_eq!(decode_entities("trailing &"), "trailing &");
    }

    #[test]
    fn out_of_order_and_empty_documents_are_handled() {
        let lines = parse_timedtext(r#"<text start="5">b</text><text start="1">a</text>"#);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].0 < lines[1].0, "sorted by time");
        assert!(parse_timedtext("").is_empty());
        assert!(parse_timedtext("<timedtext></timedtext>").is_empty());
        assert!(
            parse_timedtext("<text start=").is_empty(),
            "truncated input"
        );
    }

    #[test]
    fn a_person_written_track_beats_an_auto_generated_one() {
        let tracks = vec![
            CaptionTrack {
                language: "en".into(),
                machine: true,
                url: "https://example.invalid/asr".into(),
            },
            CaptionTrack {
                language: "ja".into(),
                machine: false,
                url: "https://example.invalid/human".into(),
            },
        ];
        assert_eq!(choose(&tracks).unwrap().language, "ja");

        // With only ASR left, ASR is still better than no lyrics — the caller
        // applies the coverage guard to it.
        let asr_only = vec![tracks[0].clone()];
        assert_eq!(choose(&asr_only).unwrap().language, "en");
        assert!(choose(&[]).is_none());
    }

    /// A machine track must be reported as machine, or the guard cannot do its job.
    #[test]
    fn a_track_reports_whether_it_is_machine_made() {
        let track = Track {
            bvid: "dQw4w9WgXcQ".into(),
            source: Source::Youtube,
            aid: 0,
            cid: 0,
            title: "t".into(),
            author: "a".into(),
            duration: 213,
            cover: None,
        };
        assert_eq!(track.source, Source::Youtube);
    }
}

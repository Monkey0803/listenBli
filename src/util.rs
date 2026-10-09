//! Small pure helpers shared by the API, lyrics and UI layers.
//!
//! Everything here is deliberately free of I/O so it can be unit tested.

/// Remove HTML tags and decode the entities Bilibili embeds in titles.
pub fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    unescape_html(&out).trim().to_string()
}

/// Decode the handful of HTML entities that show up in titles.
pub fn unescape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // Find the terminating ';' within a sane distance.
        let limit = (i + 12).min(chars.len());
        match chars[i + 1..limit].iter().position(|&c| c == ';') {
            Some(offset) => {
                let entity: String = chars[i + 1..i + 1 + offset].iter().collect();
                match decode_entity(&entity) {
                    Some(decoded) => {
                        out.push_str(&decoded);
                        i += offset + 2;
                    }
                    None => {
                        out.push(chars[i]);
                        i += 1;
                    }
                }
            }
            None => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

fn decode_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => return Some("&".into()),
        "lt" => return Some("<".into()),
        "gt" => return Some(">".into()),
        "quot" => return Some("\"".into()),
        "apos" => return Some("'".into()),
        "nbsp" => return Some(" ".into()),
        _ => {}
    }
    let code = if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        entity.strip_prefix('#')?.parse::<u32>().ok()?
    };
    char::from_u32(code).map(String::from)
}

/// Protocol-relative Bilibili URLs (`//i2.hdslb.com/...`) become absolute, and
/// plain HTTP is upgraded so mixed content never reaches the image decoder.
pub fn normalize_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    if let Some(rest) = url.strip_prefix("//") {
        Some(format!("https://{rest}"))
    } else if let Some(rest) = url.strip_prefix("http://") {
        Some(format!("https://{rest}"))
    } else {
        Some(url.to_string())
    }
}

/// Bilibili's image CDN accepts a `@<w>w_<h>h_1c.webp` suffix for a small
/// variant; the loader falls back to the original if this fails.
pub fn cover_thumbnail_url(url: &str, size: u32) -> String {
    if url.contains('@') {
        return url.to_string();
    }
    format!("{url}@{size}w_{size}h_1c.webp")
}

/// `"5:17"` -> 317, `"1:02:03"` -> 3723. Unparsable input yields 0.
pub fn parse_duration(input: &str) -> u64 {
    let input = input.trim();
    if input.is_empty() {
        return 0;
    }
    let mut total = 0u64;
    let mut ok = false;
    for part in input.split(':') {
        match part.trim().parse::<u64>() {
            Ok(value) => {
                total = total.saturating_mul(60).saturating_add(value);
                ok = true;
            }
            Err(_) => return 0,
        }
    }
    if ok {
        total
    } else {
        0
    }
}

/// 317 -> `"05:17"`, 3723 -> `"1:02:03"`.
pub fn format_duration(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Noise that uploaders prepend/append to song titles, which we must remove
/// before querying a third-party lyrics provider.
const NOISE_TOKENS: &[&str] = &[
    "官方",
    "MV",
    "mv",
    "高音质",
    "无损音质",
    "无损",
    "音质",
    "Hi-Res",
    "HIRES",
    "hires",
    "4K",
    "8K",
    "修复",
    "完整版",
    "全曲",
    "动态歌词",
    "歌词版",
    "音频",
    "纯音乐",
    "翻唱",
    "原唱",
    "钢琴版",
    "cover",
    "Cover",
    "COVER",
    "Official",
    "official",
    "Music Video",
    "中文字幕",
    "中日双语",
    "珍藏版",
    "珍藏",
    "经典",
    "怀旧",
    "循环",
    "歌单",
    "合集",
    "超清",
    "高清",
    "1080P",
    "720P",
    "字幕版",
];

/// Characters that separate the song name from a description. They become spaces
/// rather than truncation points: truncating would delete the song name entirely
/// whenever an uploader leads with a separator (e.g. `｜《晴天》- 周杰伦`).
fn is_separator(c: char) -> bool {
    matches!(
        c,
        '|' | '丨'
            | '｜'
            | '~'
            | '～'
            | '/'
            | '\\'
            | '-'
            | '—'
            | '–'
            | '_'
            | '·'
            | '、'
            | '，'
            | ','
            | '.'
            | '。'
            | '!'
            | '！'
            | '?'
            | '？'
            | ':'
            | '：'
            | ';'
            | '；'
            | '\''
            | '"'
            | '\u{2018}'
            | '\u{2019}'
            | '\u{201C}'
            | '\u{201D}'
    )
}

/// Delete a bracketed group *and* its content. Used for decorative markers such
/// as `【4K修复】` that carry no part of the song name.
///
/// If the brackets are unbalanced the text is returned untouched, because
/// otherwise a stray `【` would silently swallow the rest of the title.
fn remove_bracketed(input: &str, open: char, close: char) -> String {
    if input.matches(open).count() != input.matches(close).count() {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    for ch in input.chars() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.saturating_sub(1);
        } else if depth == 0 {
            out.push(ch);
        }
    }
    out
}

/// Drop only the bracket characters, keeping the content. Song names are very
/// often written as `《晴天》` or `（晴天）`, so the content must survive.
fn unwrap_bracketed(input: &str, open: char, close: char) -> String {
    input
        .chars()
        .filter(|c| *c != open && *c != close)
        .collect()
}

/// Map "fancy font" letters that uploaders love (mathematical alphanumeric
/// symbols, fullwidth Latin) back to plain ASCII, so the text is searchable.
fn normalize_exotic_letters(input: &str) -> String {
    // Each block is a contiguous run of A-Z followed by a-z.
    const BLOCKS: [u32; 10] = [
        0x1D400, // bold
        0x1D434, // italic
        0x1D468, // bold italic
        0x1D4D0, // bold script
        0x1D56C, // bold fraktur
        0x1D5A0, // sans-serif
        0x1D5D4, // sans-serif bold
        0x1D608, // sans-serif italic
        0x1D63C, // sans-serif bold italic
        0x1D670, // monospace
    ];

    input
        .chars()
        .map(|c| {
            let cp = c as u32;
            // Fullwidth ASCII (！-～) -> ASCII.
            if (0xFF01..=0xFF5E).contains(&cp) {
                return char::from_u32(cp - 0xFF01 + 0x21).unwrap_or(c);
            }
            // Ideographic space.
            if cp == 0x3000 {
                return ' ';
            }
            for base in BLOCKS {
                if (base..base + 52).contains(&cp) {
                    let offset = cp - base;
                    return if offset < 26 {
                        (b'A' + offset as u8) as char
                    } else {
                        (b'a' + (offset - 26) as u8) as char
                    };
                }
            }
            c
        })
        .collect()
}

/// Produce a search query suitable for a lyrics provider.
pub fn clean_song_title(title: &str) -> String {
    let mut text = normalize_exotic_letters(&strip_html(title));

    // Decorative/technical groups go away entirely...
    for (open, close) in [('【', '】'), ('[', ']'), ('〈', '〉')] {
        text = remove_bracketed(&text, open, close);
    }
    // ...while brackets that usually *contain* the song name are only unwrapped.
    for (open, close) in [('《', '》'), ('（', '）'), ('(', ')')] {
        text = unwrap_bracketed(&text, open, close);
    }

    for token in NOISE_TOKENS {
        text = text.replace(token, " ");
    }

    let text: String = text
        .chars()
        .map(|c| if is_separator(c) { ' ' } else { c })
        .collect();

    collapse_whitespace(&text)
}

pub fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Similarity of two song-ish titles in `0.0..=1.0`.
///
/// Plain edit distance is not enough on its own: the query usually carries extra
/// context, so `"周杰伦 晴天"` should score highly against the NetEase track
/// `"晴天"` even though the strings differ substantially in length. A containment
/// bonus covers that case while the edit distance still rejects unrelated songs.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    let a = normalize_for_compare(a);
    let b = normalize_for_compare(b);
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let longest = a.chars().count().max(b.chars().count());
    let edit = 1.0 - (levenshtein(&a, &b) as f64 / longest as f64);
    edit.max(containment_similarity(&a, &b))
        .max(coverage_similarity(&a, &b))
        .clamp(0.0, 1.0)
}

fn containment_similarity(a: &str, b: &str) -> f64 {
    if !(a.contains(b) || b.contains(a)) {
        return 0.0;
    }
    let shorter = a.chars().count().min(b.chars().count()) as f64;
    let longer = a.chars().count().max(b.chars().count()) as f64;
    // A single shared character is coincidence, not a match.
    if shorter < 2.0 {
        return 0.0;
    }
    // The length ratio dominates: a short candidate inside a long query is a
    // weaker signal than an equal-length one.
    0.55 + 0.45 * (shorter / longer)
}

/// Order-insensitive recall of the shorter string's characters in the longer.
///
/// Substring containment alone is too brittle: the artist and the title get
/// swapped around (`"晴天 周杰伦"` vs `"周杰伦 晴天 2160P 版"`), which `contains`
/// can never see. Counting characters that appear in both handles that.
fn coverage_similarity(a: &str, b: &str) -> f64 {
    let (shorter, longer) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    let short_len = shorter.chars().count();
    // Two characters matching is coincidence; require a real phrase.
    if short_len < 3 {
        return 0.0;
    }

    let mut pool: Vec<char> = longer.chars().collect();
    let mut hits = 0usize;
    for c in shorter.chars() {
        if let Some(pos) = pool.iter().position(|p| *p == c) {
            pool.remove(pos);
            hits += 1;
        }
    }
    hits as f64 / short_len as f64
}

fn normalize_for_compare(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && !"·-—_()（）[]【】".contains(*c))
        .collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_entities() {
        assert_eq!(
            strip_html("<em class=\"keyword\">周杰伦</em> - 晴天 &amp; 雨"),
            "周杰伦 - 晴天 & 雨"
        );
        assert_eq!(
            unescape_html("&lt;a&gt;&#39;&quot;&#x4e2d;&#25991;&nbsp;"),
            "<a>'\"中文 "
        );
    }

    #[test]
    fn leaves_unknown_entities_alone() {
        assert_eq!(unescape_html("a &bogus; b"), "a &bogus; b");
        assert_eq!(unescape_html("100% & 50%"), "100% & 50%");
    }

    #[test]
    fn normalizes_protocol_relative_urls() {
        assert_eq!(
            normalize_url("//i2.hdslb.com/x.jpg").as_deref(),
            Some("https://i2.hdslb.com/x.jpg")
        );
        assert_eq!(
            normalize_url("http://i2.hdslb.com/x.jpg").as_deref(),
            Some("https://i2.hdslb.com/x.jpg")
        );
        assert_eq!(normalize_url("   "), None);
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("5:17"), 317);
        assert_eq!(parse_duration("1:02:03"), 3723);
        assert_eq!(parse_duration("0:04"), 4);
        assert_eq!(parse_duration(""), 0);
        assert_eq!(parse_duration("abc"), 0);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(317), "05:17");
        assert_eq!(format_duration(3723), "1:02:03");
        assert_eq!(format_duration(0), "00:00");
    }

    #[test]
    fn cleans_typical_bilibili_titles() {
        let cleaned = clean_song_title("【4K修复】周杰伦 - 晴天【高音质】");
        assert!(cleaned.contains("周杰伦"), "got {cleaned:?}");
        assert!(cleaned.contains("晴天"), "got {cleaned:?}");
        assert!(!cleaned.contains("4K"), "got {cleaned:?}");
        assert!(!cleaned.contains("【"), "got {cleaned:?}");

        // Separators become spaces instead of truncating.
        let cleaned = clean_song_title("晴天 | 周杰伦无损音质合集");
        assert!(!cleaned.contains('|'));
        assert!(cleaned.contains("晴天"));

        // Whitespace is collapsed.
        let cleaned = clean_song_title("  晴天   周杰伦  ");
        assert_eq!(cleaned, "晴天 周杰伦");
    }

    /// Regression test: a real uploader title that previously collapsed to an
    /// empty query because it began with `｜` and wrapped the song name in `《》`.
    #[test]
    fn keeps_the_song_name_from_a_separator_led_title() {
        let cleaned = clean_song_title("【𝐇𝐢-𝐑𝐞𝐬无损音质】｜《晴天》- 周杰伦 -‘故事的小黄花’");
        assert!(
            cleaned.contains("晴天"),
            "song name inside 《》 must survive: {cleaned:?}"
        );
        assert!(
            cleaned.contains("周杰伦"),
            "artist must survive: {cleaned:?}"
        );
        assert!(
            !cleaned.contains('｜'),
            "separators must not remain: {cleaned:?}"
        );
        assert!(
            !cleaned.contains('【'),
            "decoration must be removed: {cleaned:?}"
        );
        assert!(!cleaned.is_empty(), "the query must never be empty");
    }

    #[test]
    fn brackets_containing_the_song_name_are_unwrapped_not_deleted() {
        assert!(clean_song_title("《晴天》").contains("晴天"));
        assert!(clean_song_title("（晴天）").contains("晴天"));
        assert!(clean_song_title("(晴天)").contains("晴天"));
        // Decorative groups are still removed entirely.
        assert_eq!(clean_song_title("【晴天】"), "");
    }

    #[test]
    fn normalizes_fancy_unicode_letters() {
        let cleaned = clean_song_title("𝐇𝐞𝐥𝐥𝐨 Ｗｏｒｌｄ");
        assert!(
            cleaned.contains("Hello") && cleaned.contains("World"),
            "got {cleaned:?}"
        );
    }

    #[test]
    fn unmatched_brackets_do_not_swallow_the_title() {
        // An opening bracket with no closer must not discard the rest.
        let cleaned = clean_song_title("【未闭合 晴天");
        assert!(!cleaned.is_empty());
    }

    #[test]
    fn similarity_prefers_the_right_song() {
        let query = clean_song_title("【4K修复】周杰伦 - 晴天");
        let exact = title_similarity(&query, "晴天");
        let other = title_similarity(&query, "稻香");
        assert!(
            exact > other,
            "expected the matching title to score higher ({exact} vs {other})"
        );
    }

    #[test]
    fn similarity_ignores_artist_context_in_the_query() {
        // The query keeps the artist; the provider's track name does not.
        let query = "周杰伦 晴天";
        let score = title_similarity(query, "晴天");
        assert!(score >= 0.7, "containment match too weak: {score}");
        assert!(score < 1.0, "should not be a perfect match: {score}");
    }

    #[test]
    fn similarity_rejects_unrelated_and_coincidental_matches() {
        assert!(title_similarity("周杰伦 晴天", "稻香") < 0.5);
        // A single shared character must not count as a match.
        assert!(title_similarity("周杰伦 晴天", "雨") < 0.5);
    }

    #[test]
    fn similarity_of_identical_titles_is_one() {
        assert!((title_similarity("晴天", "晴天") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn thumbnail_url_appends_size_hint_once() {
        let url = "https://i2.hdslb.com/x.jpg";
        let thumb = cover_thumbnail_url(url, 120);
        assert!(thumb.ends_with("@120w_120h_1c.webp"));
        assert_eq!(cover_thumbnail_url(&thumb, 120), thumb);
    }
}

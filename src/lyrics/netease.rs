//! Lyrics fallback via NetEase Cloud Music's public search/lyric endpoints.
//!
//! This is a third-party, non-contractual API. It is treated as a best-effort
//! enhancement: any failure degrades to "no lyrics" and never blocks playback.
//!
//! Matching is the interesting part. Bilibili uploader names are usually *not*
//! the performing artist, so the artist field only contributes a small bonus
//! while the cleaned song title dominates, with the track duration breaking
//! ties.

use super::lrc::{self, Lyrics, LyricsSource};
use crate::api::client::{Api, ApiError};
use crate::api::models::{NeSong, Track};
use crate::util;

/// Minimum title similarity before a NetEase result is considered a match.
const MIN_TITLE_SCORE: f64 = 0.5;

/// How many ranked candidates to try fetching lyrics for.
///
/// The best-scoring song is sometimes an instrumental or a re-upload with an
/// empty lyric, so falling through to the next candidate is what makes lyric
/// lookup actually reliable.
const MAX_LYRIC_ATTEMPTS: usize = 3;

pub fn fetch(api: &Api, track: &Track) -> Result<Option<Lyrics>, ApiError> {
    let queries = search_queries(&track.title);
    if queries.is_empty() {
        return Ok(None);
    }

    // NetEase's public search returns covers rather than the official release for
    // most licensed Chinese songs, and a very specific query can return almost
    // nothing. Searching a couple of query shapes and pooling the candidates
    // makes matching far more reliable.
    let mut songs: Vec<NeSong> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut last_error: Option<ApiError> = None;

    for query in &queries {
        match api.netease_search(query) {
            Ok(response) => {
                for song in response.result.map(|inner| inner.songs).unwrap_or_default() {
                    if seen.insert(song.id) {
                        songs.push(song);
                    }
                }
            }
            Err(err) => last_error = Some(err),
        }
    }

    let candidates = rank_candidates(&track.title, track.duration, &track.author, &songs);
    for song in candidates.iter().take(MAX_LYRIC_ATTEMPTS) {
        match api.netease_lyric(song.id) {
            Ok(lyric) => {
                if let Some(lyrics) = build_lyrics(lyric)? {
                    return Ok(Some(lyrics));
                }
                // No lyric for this candidate; try the next one.
            }
            Err(err) => last_error = Some(err),
        }
    }

    if songs.is_empty() {
        if let Some(err) = last_error {
            return Err(err);
        }
    }
    Ok(None)
}

fn build_lyrics(lyric: crate::api::models::NeLyric) -> Result<Option<Lyrics>, ApiError> {
    let main = lrc::parse_lrc(lyric.lrc.map(|l| l.lyric).unwrap_or_default().as_str());
    if main.is_empty() {
        return Ok(None);
    }
    let translation = lyric
        .tlyric
        .map(|t| lrc::parse_lrc(&t.lyric))
        .unwrap_or_default();

    Ok(Some(Lyrics {
        lines: lrc::merge_translation(main, translation),
        source: LyricsSource::Netease,
    }))
}

/// Clean the uploader's title into something a lyrics provider can match.
pub fn build_query(title: &str) -> String {
    let cleaned = util::clean_song_title(title);
    if cleaned.trim().is_empty() {
        util::strip_html(title)
    } else {
        cleaned
    }
}

/// The query shapes we try, most specific first.
///
/// Uploaders frequently append a lyric snippet after the artist
/// (`晴天 周杰伦 故事的小黄花`), which narrows the search to uselessness; the
/// two-token prefix recovers the good matches.
pub fn search_queries(title: &str) -> Vec<String> {
    let primary = build_query(title);
    let mut queries: Vec<String> = Vec::new();
    if primary.is_empty() {
        return queries;
    }
    queries.push(primary.clone());

    let tokens: Vec<&str> = primary.split_whitespace().collect();
    if tokens.len() > 2 {
        let short = tokens[..2].join(" ");
        if !queries.contains(&short) {
            queries.push(short);
        }
    }
    queries
}

/// Candidates at or above the match threshold, best first.
pub fn rank_candidates<'a>(
    title: &str,
    duration_secs: u64,
    author: &str,
    songs: &'a [NeSong],
) -> Vec<&'a NeSong> {
    let query = build_query(title);
    if query.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(f64, &NeSong)> = Vec::new();
    for song in songs {
        // Normalize both sides identically: the provider's track name carries
        // its own decorations ("晴天(原唱 周杰伦)", "晴天 R&B版").
        let candidate = build_query(&song.name);
        if candidate.is_empty() {
            continue;
        }

        let title_score = util::title_similarity(&query, &candidate);
        if title_score < MIN_TITLE_SCORE {
            continue;
        }
        // Weak signal on purpose: uploader names rarely match artist names.
        let artist_score = song
            .artists
            .iter()
            .map(|artist| util::title_similarity(author, &artist.name))
            .fold(0.0f64, f64::max);
        let duration_score = duration_similarity(duration_secs, song.duration);

        scored.push((
            title_score * 0.75 + duration_score * 0.2 + artist_score * 0.05,
            song,
        ));
    }

    // Stable ordering: ties keep the provider's own relevance order.
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().map(|(_, song)| song).collect()
}

/// The single best match, or `None` when nothing is close enough.
pub fn pick_best<'a>(
    title: &str,
    duration_secs: u64,
    author: &str,
    songs: &'a [NeSong],
) -> Option<&'a NeSong> {
    rank_candidates(title, duration_secs, author, songs)
        .into_iter()
        .next()
}

/// `duration_ms` is NetEase's millisecond field.
///
/// Uses a *relative* tolerance on purpose: Bilibili uploads routinely pad a song
/// with an intro, an outro or a spoken segment, so a 269 s song under a 317 s
/// video is still the right match while a 112 s track is not.
fn duration_similarity(duration_secs: u64, duration_ms: u64) -> f64 {
    if duration_secs == 0 || duration_ms == 0 {
        return 0.5;
    }
    let other_secs = duration_ms / 1000;
    let delta = (duration_secs as i64 - other_secs as i64).unsigned_abs() as f64;
    let ratio = delta / duration_secs.max(1) as f64;
    // Full credit at an exact match, zero once the lengths differ by 35%.
    (1.0 - ratio / 0.35).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::NeArtist;

    fn song(id: i64, name: &str, artists: &[&str], duration_ms: u64) -> NeSong {
        NeSong {
            id,
            name: name.to_string(),
            artists: artists
                .iter()
                .map(|a| NeArtist {
                    name: a.to_string(),
                })
                .collect(),
            album: None,
            duration: duration_ms,
        }
    }

    #[test]
    fn query_is_cleaned_but_never_empty() {
        assert!(build_query("【4K修复】周杰伦 - 晴天").contains("晴天"));
        // A title made entirely of noise still yields something usable.
        assert!(!build_query("【高音质】").trim().is_empty());
    }

    #[test]
    fn search_queries_shorten_a_title_carrying_a_lyric_snippet() {
        let queries = search_queries("【𝐇𝐢-𝐑𝐞𝐬无损音质】｜《晴天》- 周杰伦 -‘故事的小黄花’");
        assert!(!queries.is_empty(), "must produce at least one query");
        assert!(queries[0].contains("晴天"), "got {queries:?}");
        // A specific query can return almost nothing, so a shorter one is tried.
        assert_eq!(
            queries.len(),
            2,
            "expected a shortened fallback: {queries:?}"
        );
        assert!(
            queries[1].contains("晴天") && queries[1].contains("周杰伦"),
            "got {queries:?}"
        );
    }

    #[test]
    fn search_queries_do_not_duplicate_a_short_title() {
        assert_eq!(search_queries("晴天"), vec!["晴天".to_string()]);
        assert_eq!(
            search_queries("周杰伦 晴天"),
            vec!["周杰伦 晴天".to_string()]
        );
        assert!(search_queries("晴天").len() == 1);
        // Even an all-noise title degrades to the raw text rather than nothing.
        assert!(!search_queries("【高音质】").is_empty());
    }

    /// The official release is usually absent from NetEase's public search, so a
    /// cover carrying the same song name (and the right duration) must win.
    #[test]
    fn matches_a_cover_whose_name_carries_the_artist() {
        let songs = vec![
            song(1, "晴天 粤语版", &["陈与默"], 296_904),
            song(2, "晴天（cover：周杰伦）", &["蟹老板要高冷"], 135_581),
            song(3, "晴天 (原唱 周杰伦)", &["RyaVocal"], 270_738),
        ];
        let best = pick_best(
            "【𝐇𝐢-𝐑𝐞𝐬无损音质】｜《晴天》- 周杰伦 -‘故事的小黄花’",
            270,
            "",
            &songs,
        )
        .expect("a cover should match");
        assert_eq!(
            best.id, 3,
            "expected the closest-duration cover to win, got {:?}",
            best.name
        );
    }

    #[test]
    fn prefers_the_matching_title() {
        let songs = vec![
            song(1, "稻香", &["周杰伦"], 223_000),
            song(2, "晴天", &["周杰伦"], 269_000),
        ];
        let best = pick_best("【4K修复】周杰伦 - 晴天", 269, "音乐无限", &songs).unwrap();
        assert_eq!(best.name, "晴天");
    }

    #[test]
    fn duration_breaks_ties_between_same_title() {
        let songs = vec![
            song(1, "晴天", &["周杰伦"], 269_000),
            song(2, "晴天 (Live)", &["周杰伦"], 300_000),
        ];
        let best = pick_best("晴天", 269, "", &songs).unwrap();
        assert_eq!(best.id, 1);
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let songs = vec![song(1, "完全不同的歌名", &["某人"], 200_000)];
        assert!(pick_best("晴天", 269, "", &songs).is_none());
        assert!(pick_best("晴天", 269, "", &[]).is_none());
    }

    #[test]
    fn candidates_are_ranked_best_first_and_pick_best_agrees() {
        let songs = vec![
            song(1, "晴天 粤语版", &["陈与默"], 296_904),
            song(2, "晴天 (原唱 周杰伦)", &["RyaVocal"], 270_738),
            song(3, "完全无关的歌", &["某人"], 200_000),
        ];
        let ranked = rank_candidates("晴天 周杰伦", 270, "", &songs);
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].id, 2, "best candidate should come first");
        assert!(
            !ranked.iter().any(|s| s.id == 3),
            "unrelated songs must be filtered out"
        );
        assert_eq!(pick_best("晴天 周杰伦", 270, "", &songs).unwrap().id, 2);
    }

    #[test]
    fn ranking_is_empty_without_a_usable_query() {
        let songs = vec![song(1, "晴天", &["周杰伦"], 269_000)];
        assert!(rank_candidates("", 0, "", &songs).is_empty());
        assert!(rank_candidates("晴天", 0, "", &[]).is_empty());
    }

    #[test]
    fn uploader_name_does_not_override_a_good_title_match() {
        // The uploader is a music channel, not the artist.
        let songs = vec![song(1, "晴天", &["周杰伦"], 269_000)];
        let best = pick_best("晴天", 269, "zyl2012_音乐无限", &songs).unwrap();
        assert_eq!(best.id, 1);
    }

    #[test]
    fn duration_similarity_is_relative() {
        // Near-exact matches score highest.
        assert!(duration_similarity(200, 203_000) > 0.9);
        // A modest pad is still a good match...
        assert!(duration_similarity(200, 212_000) > 0.8);
        // ...but a wildly different length is not.
        assert_eq!(duration_similarity(200, 400_000), 0.0);
        // Unknown durations stay neutral instead of penalising.
        assert_eq!(duration_similarity(0, 200_000), 0.5);
        assert_eq!(duration_similarity(200, 0), 0.5);
    }

    /// A Bilibili upload pads the song (317 s video, 269 s track), which must
    /// still beat a much shorter unrelated track of the same name.
    #[test]
    fn padded_video_still_prefers_the_right_length_track() {
        let songs = vec![
            song(1, "晴天", &["Jay"], 112_373),
            song(2, "晴天 (原唱 周杰伦)", &["RyaVocal"], 270_738),
        ];
        let best = pick_best("【4K修复】周杰伦 - 晴天MV 2160P修复版", 317, "", &songs).unwrap();
        assert_eq!(
            best.id, 2,
            "expected the full-length track, got {}",
            best.name
        );
    }
}

//! Setup shared by the snapshot examples.
//!
//! Both of them need the same thing before they can draw anything: an isolated
//! home, and a library with something in it. Keeping that here is what makes the
//! two renderers show the same interface.

use listenbli::api::models::{Source, Track};
use listenbli::app::App;
use listenbli::library::Playlist;
use listenbli::platform;

/// Points `HOME` at `LISTENBLI_HOME` when it is set.
///
/// A seeded snapshot must never touch the real config directory: it would add
/// invented songs to somebody's actual library.
pub fn isolate_home() {
    if let Ok(home) = std::env::var("LISTENBLI_HOME") {
        std::env::set_var("HOME", &home);
        let _ = std::fs::create_dir_all(&home);
    }
}

pub fn seed_library(app: &mut App, page: &str) {
    #[allow(clippy::too_many_arguments)] // a table row, written out in full
    fn track(
        bvid: &str,
        title: &str,
        author: &str,
        artist: Option<&str>,
        album: Option<&str>,
        duration: u64,
        source: Source,
        cid: i64,
    ) -> Track {
        Track {
            bvid: bvid.to_owned(),
            source,
            aid: cid,
            cid,
            title: title.to_owned(),
            author: author.to_owned(),
            duration,
            cover: None,
            album: album.map(str::to_owned),
            artist: artist.map(str::to_owned),
        }
    }

    let songs = vec![
        track(
            "BV1Jt411k7Qd",
            "晴天",
            "杰威尔音乐",
            Some("周杰伦"),
            Some("叶惠美"),
            269,
            Source::Bilibili,
            1,
        ),
        track(
            "BV1dd4y1c7Wp",
            "七里香",
            "杰威尔音乐",
            Some("周杰伦"),
            Some("七里香"),
            299,
            Source::Bilibili,
            2,
        ),
        track(
            "BV1nL411x7Tg",
            "稻香",
            "华语音乐馆",
            Some("周杰伦"),
            Some("魔杰座"),
            223,
            Source::Bilibili,
            3,
        ),
        track(
            "9K3FQk9dS7o",
            "告白气球",
            "杰威爾音樂 JVR Music",
            Some("周杰伦"),
            None,
            215,
            Source::Youtube,
            4,
        ),
        track(
            "BV1Qs4y1R7Jk",
            "夜曲",
            "音乐现场 Live",
            Some("周杰伦"),
            Some("十一月的萧邦"),
            227,
            Source::Bilibili,
            5,
        ),
        track(
            "2uJ4n1kL0xM",
            "青花瓷",
            "杰威爾音樂 JVR Music",
            Some("周杰伦"),
            Some("我很忙"),
            239,
            Source::Youtube,
            6,
        ),
        track(
            "BV1xT4y1Z7Hd",
            "晴天（钢琴改编版）",
            "钢琴小妹 Piano",
            None,
            None,
            312,
            Source::Bilibili,
            7,
        ),
        track(
            "BV1qP4y1t7Rn",
            "周杰伦 经典歌曲合集 50 首",
            "华语音乐搬运工",
            None,
            None,
            4532,
            Source::Bilibili,
            8,
        ),
    ];
    for song in songs {
        app.library_mut().add(song);
    }
    // And the same recording on the other platform, which is the case 来源详情
    // exists for: one song, two copies, one of them playing.
    app.library_mut().add(track(
        "9K3FQk9dS7o-twin",
        "晴天",
        "JVR Music",
        Some("周杰伦"),
        Some("叶惠美"),
        269,
        Source::Youtube,
        99,
    ));

    let key_of = listenbli::library::LocalLibrary::key_of;
    let keys: Vec<String> = app
        .library()
        .entries
        .iter()
        .map(|entry| key_of(&entry.track))
        .collect();
    // Two different keys on purpose: the library key (`key_of`) is what playlists
    // and 最近播放 reference, while the cache names its files after `cache_key`.
    let cache_keys: Vec<String> = app
        .library()
        .entries
        .iter()
        .map(|entry| entry.track.cache_key())
        .collect();

    {
        let library = app.library_mut();
        let mut local = Playlist::local("late-night", "深夜循环");
        local.entries = keys.iter().take(4).cloned().collect();
        library.playlists.push(local);
    }

    // Through the library's own sync, so 来源与同步 shows a real ledger rather
    // than a hand-written playlist.
    app.library_mut().sync_playlist(
        "platform:bilibili:1",
        "默认收藏夹",
        Source::Bilibili,
        keys.iter().skip(1).take(5).cloned().collect(),
    );

    // Local copies, so 已下载 has groups and both of its states: two finished
    // files and one half-written fragment.
    let cache = listenbli::audio::cache::AudioCache::at(&platform::cache_dir());
    // `store`/`begin` create this; a bare write does not.
    let _ = std::fs::create_dir_all(cache.dir());
    for key in cache_keys.iter().take(2) {
        let _ = std::fs::write(cache.ready_path_for(key, 30280), b"seed");
    }
    if let Some(key) = cache_keys.get(2) {
        let _ = cache.begin(key, 30280, 4_000_000);
    }

    // A few plays spanning two days, so 最近播放 has its 今天 / 昨天 groups.
    let (today, yesterday) = listenbli::library::today_and_yesterday();
    let recent: Vec<listenbli::library::RecentPlay> = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(n, key)| listenbli::library::RecentPlay {
            key: key.clone(),
            at: format!("{} 21:0{n}", if n < 2 { &today } else { &yesterday }),
        })
        .collect();
    app.library_mut().recent = recent;

    // The index is built when the app starts, which was before any of this.
    app.refresh_cache_index();

    // A search of the same artist: three hits that are already in the library and
    // two that are not, which is the whole point of the page.
    {
        let mut results: Vec<Track> = app
            .library()
            .entries
            .iter()
            .take(3)
            .map(|entry| entry.track.clone())
            .collect();
        results.push(track(
            "BV1newone0001",
            "龙卷风",
            "杰威尔音乐",
            Some("周杰伦"),
            Some("Jay"),
            232,
            Source::Bilibili,
            21,
        ));
        results.push(track(
            "yt-newone0002",
            "安静",
            "JVR Music",
            Some("周杰伦"),
            Some("范特西"),
            330,
            Source::Youtube,
            22,
        ));
        app.show_search_results("周杰伦", results);
    }

    // One song in the player, so 正在播放 and the deck have something to show.
    if let Some(entry) = app.library().entries.first() {
        app.show_as_playing(
            entry.track.clone(),
            Some(listenbli::api::models::AudioQuality::Flac),
        );
    }

    // `LISTENBLI_OPEN=fallback` plays the *other* edition of a song whose first
    // choice is gone, which is the one state that takes a failing platform to
    // reach and therefore the one a screenshot cannot otherwise show.
    // A source somebody declared by hand, which is what 来源与同步 has to be able
    // to show without pretending it works.
    if page.contains("declaration") {
        app.declare(listenbli::config::Declaration {
            id: "archive-org".to_owned(),
            name: "Internet Archive".to_owned(),
            channel: "I".to_owned(),
            capabilities: ["search", "stream", "cache", "anon"]
                .iter()
                .map(|key| (*key).to_owned())
                .collect(),
            session: listenbli::config::Session::Anonymous,
        });
    }

    if page.starts_with("fallback") {
        if let Some(entry) = app.library().entries.first().cloned() {
            app.mark_dead(&entry.track);
            let other = entry
                .versions
                .iter()
                .find(|track| track.key() != entry.track.key())
                .cloned()
                .unwrap_or(entry.track);
            app.show_as_playing(other, Some(listenbli::api::models::AudioQuality::YtAac128));
        }
    }
}

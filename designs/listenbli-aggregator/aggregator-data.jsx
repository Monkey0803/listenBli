// aggregator-data.jsx — mock content for the aggregated listenBli prototype.
//
// Shapes follow src/api/models.rs (Track / Source / AudioQuality) and src/net.rs
// (per-source download plan, cache completeness). The prototype needs one thing
// the real model does not have yet: an **entry** that groups the several editions
// of the same recording across sources. That grouping is the whole design problem
// this round is about, so it is modelled explicitly:
//
//   ENTRY  (the song)        晴天 — 周杰伦
//     └ EDITION (a stream)   A·B站 【4K修复】Official MV   FLAC 无损  已缓存
//                            B·YT  周杰倫【晴天】官方 MV     AAC 130K  未缓存
//
// Everything below is demo content, not measured data.

// ---------------------------------------------------------------------------
// Sources. Each direction picks the identity facet it needs; none of them may
// use a brand colour as a primary interface colour (see DESIGN-NOTES.md).
// ---------------------------------------------------------------------------

const SOURCES = {
  bilibili: {
    id: "bilibili",
    name: "哔哩哔哩",
    short: "B 站",
    channel: "A", // instrument-panel channel letter
    stare: "B 站", // editorial stamp
    filament: "BL", // two-letter mark
    hue: 348, // tide filament hue (degrees)
    session: { kind: "account", label: "夜航西飞", detail: "大会员", ok: true },
    caps: ["搜索", "收藏夹", "历史", "无损 FLAC", "CC 字幕", "扫码登录"],
  },
  youtube: {
    id: "youtube",
    name: "YouTube",
    short: "YouTube",
    channel: "B",
    stare: "YT",
    filament: "YT",
    hue: 14,
    session: { kind: "anonymous", label: "匿名访问", detail: "visitorData 已取", ok: true },
    caps: ["搜索", "字幕（含 ASR）", "AAC 130K", "无需登录"],
  },
};

const SOURCE_ORDER = ["bilibili", "youtube"];

// ---------------------------------------------------------------------------
// The search result, already merged across sources.
// ---------------------------------------------------------------------------

const QUERY = { keyword: "周杰伦 晴天", sources: 2, took: 0.42 };

const ENTRIES = [
  {
    id: "qingtian",
    title: "晴天",
    artist: "周杰伦",
    album: "叶惠美",
    year: 2003,
    duration: 269,
    initial: "晴",
    editions: [
      {
        source: "bilibili",
        vid: "BV1Jt411k7Qd",
        variant: "【4K修复】周杰伦 - 晴天 (Official MV)",
        quality: "无损 FLAC",
        kbps: 1411,
        duration: 269,
        views: "412.8 万",
        uploader: "杰威尔音乐",
        cache: { done: true, mb: 38.2 },
      },
      {
        source: "youtube",
        vid: "dQw4w9WgXcQ",
        variant: "周杰倫 Jay Chou【晴天 Sunny Day】官方 MV",
        quality: "AAC 130K",
        kbps: 130,
        duration: 271,
        views: "1.24 亿",
        uploader: "杰威爾音樂 JVR Music",
        cache: { done: false, mb: 4.4 },
      },
    ],
    lyrics: { source: "bilibili-cc", label: "B 站 CC 字幕", confidence: "人工校对" },
  },
  {
    id: "qilixiang",
    title: "七里香",
    artist: "周杰伦",
    album: "七里香",
    year: 2004,
    duration: 299,
    initial: "七",
    editions: [
      {
        source: "bilibili",
        vid: "BV1dd4y1c7Wp",
        variant: "周杰伦《七里香》完整版",
        quality: "无损 FLAC",
        kbps: 1411,
        duration: 299,
        views: "268.1 万",
        uploader: "杰威尔音乐",
        cache: { done: true, mb: 42.6 },
      },
      {
        source: "youtube",
        vid: "BCH1cHf7LzA",
        variant: "周杰倫【七里香 Common Jasmine Orange】官方 MV",
        quality: "AAC 130K",
        kbps: 130,
        duration: 301,
        views: "5.4 千万",
        uploader: "杰威爾音樂 JVR Music",
        cache: { done: false, mb: 4.9 },
      },
    ],
    lyrics: { source: "netease", label: "网易云音乐", confidence: "匹配所得" },
  },
  {
    id: "daoxiang",
    title: "稻香",
    artist: "周杰伦",
    album: "魔杰座",
    year: 2008,
    duration: 223,
    initial: "稻",
    editions: [
      {
        source: "bilibili",
        vid: "BV1nL411x7Tg",
        variant: "【无损音质】稻香 - 周杰伦",
        quality: "无损 FLAC",
        kbps: 1411,
        duration: 223,
        views: "189.4 万",
        uploader: "华语音乐馆",
        cache: { done: true, mb: 31.8 },
      },
    ],
    lyrics: { source: "netease", label: "网易云音乐", confidence: "匹配所得" },
  },
  {
    id: "gaobaiqiqiu",
    title: "告白气球",
    artist: "周杰伦",
    album: "周杰伦的床边故事",
    year: 2016,
    duration: 215,
    initial: "告",
    editions: [
      {
        source: "youtube",
        vid: "9K3FQk9dS7o",
        variant: "周杰倫【告白氣球 Love Confession】官方 MV",
        quality: "AAC 130K",
        kbps: 130,
        duration: 215,
        views: "2.71 亿",
        uploader: "杰威爾音樂 JVR Music",
        cache: { done: false, mb: 3.6 },
      },
    ],
    lyrics: { source: "youtube-asr", label: "YouTube 自动字幕", confidence: "机器转写" },
  },
  {
    id: "yeqv",
    title: "夜曲",
    artist: "周杰伦",
    album: "十一月的萧邦",
    year: 2005,
    duration: 227,
    initial: "夜",
    editions: [
      {
        source: "bilibili",
        vid: "BV1Qs4y1R7Jk",
        variant: "夜曲 - 周杰伦（超高音质）",
        quality: "192K AAC-LC",
        kbps: 192,
        duration: 227,
        views: "96.2 万",
        uploader: "音乐现场 Live",
        cache: { done: false, mb: 5.4 },
      },
      {
        source: "youtube",
        vid: "6Q0Z1kQ1TfE",
        variant: "周杰倫【夜曲 Nocturne】Official Audio",
        quality: "AAC 130K",
        kbps: 130,
        duration: 228,
        views: "3.1 千万",
        uploader: "周杰倫 Jay Chou",
        cache: { done: false, mb: 3.7 },
      },
    ],
    lyrics: { source: "netease", label: "网易云音乐", confidence: "匹配所得" },
  },
  {
    id: "qinghuaci",
    title: "青花瓷",
    artist: "周杰伦",
    album: "我很忙",
    year: 2007,
    duration: 239,
    initial: "青",
    editions: [
      {
        source: "youtube",
        vid: "2uJ4n1kL0xM",
        variant: "周杰倫【青花瓷 Blue and White Porcelain】官方 MV",
        quality: "AAC 130K",
        kbps: 130,
        duration: 239,
        views: "9.8 千万",
        uploader: "杰威爾音樂 JVR Music",
        cache: { done: true, mb: 4.2 },
      },
    ],
    lyrics: { source: "bilibili-cc", label: "B 站 CC 字幕", confidence: "人工校对" },
  },
  {
    id: "qingtian-piano",
    title: "晴天（钢琴改编版）",
    artist: "Animenz Piano",
    album: "Animenz 编曲集",
    year: 2019,
    duration: 312,
    initial: "琴",
    editions: [
      {
        source: "bilibili",
        vid: "BV1xT4y1Z7Hd",
        variant: "【钢琴】晴天 - 演奏版",
        quality: "192K AAC-LC",
        kbps: 192,
        duration: 312,
        views: "58.3 万",
        uploader: "钢琴小妹 Piano",
        cache: { done: false, mb: 7.1 },
      },
    ],
    lyrics: null,
  },
  {
    id: "jay-collection",
    title: "周杰伦 经典歌曲合集 50 首",
    artist: "华语音乐搬运工",
    album: null,
    year: 2022,
    duration: 4532,
    initial: "合",
    editions: [
      {
        source: "bilibili",
        vid: "BV1qP4y1t7Rn",
        variant: "【Hi-Res】经典歌曲合集 50 首",
        quality: "无损 FLAC",
        kbps: 1411,
        duration: 4532,
        views: "1204 万",
        uploader: "华语音乐搬运工",
        cache: { done: false, mb: 181.9, partial: true },
      },
    ],
    lyrics: null,
  },
];

// ---------------------------------------------------------------------------
// Derived views the directions render.
// ---------------------------------------------------------------------------

/** Every edition as its own row, in entry order — what "合并重复：关" shows. */
const flatRows = () =>
  ENTRIES.flatMap((entry) =>
    entry.editions.map((edition, index) => ({ entry, edition, index }))
  );

const mergedGroupCount = () => ENTRIES.filter((e) => e.editions.length > 1).length;
const editionCount = () => ENTRIES.reduce((n, e) => n + e.editions.length, 0);

/** The edition the app would play by default: cached first, then best bitrate. */
const bestEdition = (entry) =>
  [...entry.editions].sort((a, b) => {
    if (a.cache.done !== b.cache.done) return a.cache.done ? -1 : 1;
    return b.kbps - a.kbps;
  })[0];

// ---------------------------------------------------------------------------
// Queue — deliberately cross-source, so every row has to carry its origin.
// ---------------------------------------------------------------------------

const QUEUE = [
  { title: "晴天", artist: "周杰伦", source: "bilibili", duration: 269, state: "playing", origin: "搜索" },
  { title: "七里香", artist: "周杰伦", source: "youtube", duration: 301, state: "next", origin: "下一首播放" },
  { title: "夜曲", artist: "周杰伦", source: "bilibili", duration: 227, state: "queued", origin: "搜索" },
  { title: "告白气球", artist: "周杰伦", source: "youtube", duration: 215, state: "queued", origin: "历史" },
  { title: "青花瓷", artist: "周杰伦", source: "youtube", duration: 239, state: "queued", origin: "收藏夹" },
  { title: "稻香", artist: "周杰伦", source: "bilibili", duration: 223, state: "queued", origin: "收藏夹" },
];

// ---------------------------------------------------------------------------
// Cache / offline — the six-capability brief asks this to be visible.
// ---------------------------------------------------------------------------

const CACHE = {
  totalMb: 428.6,
  limitMb: 8192,
  tracks: 17,
  partial: 1,
  folder: "~/Library/Caches/listenBli",
  bySource: { bilibili: 356.2, youtube: 72.4 },
  lyricsMb: 12.8,
};

// ---------------------------------------------------------------------------
// Capability declarations.
//
// The extensibility story made concrete: a source IS a list of declared
// capabilities, and the interface grows only what was declared — which is why a
// second source has no 收藏夹 tab today instead of an empty one. The connect
// sheet writes a declaration of exactly this shape.
// ---------------------------------------------------------------------------

const CAPS_BILIBILI = [
  { id: "search", label: "关键词搜索", on: true },
  { id: "paging", label: "分页翻页", on: true },
  { id: "stream", label: "直链解析 DASH", on: true },
  { id: "quality", label: "多档音质", on: true },
  { id: "lossless", label: "无损 FLAC", on: true },
  { id: "captions", label: "人工字幕 / CC", on: true },
  { id: "asr", label: "机器字幕", on: true },
  { id: "lyrics", label: "外部歌词匹配", on: true },
  { id: "anon", label: "匿名可用", on: true },
  { id: "account", label: "账号登录", on: true },
  { id: "favorites", label: "收藏夹", on: true },
  { id: "history", label: "观看历史", on: true },
];

const CAPS_YOUTUBE = [
  { id: "search", label: "关键词搜索", on: true },
  { id: "paging", label: "分页翻页", on: true },
  { id: "stream", label: "直链解析 itag", on: true },
  { id: "quality", label: "多档音质", on: true },
  { id: "lossless", label: "无损音源", on: false },
  { id: "captions", label: "人工字幕 / CC", on: true },
  { id: "asr", label: "机器字幕", on: true },
  { id: "lyrics", label: "外部歌词匹配", on: true },
  { id: "anon", label: "匿名可用", on: true },
  { id: "account", label: "账号登录 OAuth", on: false },
  { id: "favorites", label: "收藏夹", on: false },
  { id: "history", label: "历史记录", on: false },
];

/** The four groups the connect sheet offers, and what each capability grows. */
const CAP_GROUPS = [
  {
    group: "搜索与浏览",
    items: [
      { id: "search", label: "关键词搜索", surface: "搜索框与结果列表" },
      { id: "paging", label: "分页翻页", surface: "滚到底自动加载下一页" },
      { id: "browse", label: "分区 / 榜单浏览", surface: "分区与榜单页签" },
    ],
  },
  {
    group: "播放与音质",
    items: [
      { id: "stream", label: "直链解析", surface: "播放、队列与缓存" },
      { id: "quality", label: "多档音质", surface: "音质列与音质优先级" },
      { id: "lossless", label: "无损音源", surface: "无损徽章与优先开关" },
      { id: "cache", label: "可完整缓存", surface: "缓存与离线页" },
    ],
  },
  {
    group: "文本",
    items: [
      { id: "captions", label: "人工字幕 / CC", surface: "歌词面板出处（人工）" },
      { id: "asr", label: "机器字幕", surface: "歌词面板出处（机器）" },
      { id: "lyrics", label: "外部歌词匹配", surface: "网易云 / LRCLIB 回退" },
    ],
  },
  {
    group: "账号与库",
    items: [
      { id: "anon", label: "匿名可用", surface: "无账号也能用" },
      { id: "account", label: "账号登录", surface: "会话条与登录流程" },
      { id: "favorites", label: "收藏夹", surface: "收藏夹页签" },
      { id: "history", label: "历史记录", surface: "历史页签" },
    ],
  },
];

// ---------------------------------------------------------------------------
// What is on disk. The totals agree with CACHE above, and one item is
// part-downloaded on purpose — a half file is a real state the app must show
// rather than hide behind the same row as a complete one.
// ---------------------------------------------------------------------------

const CACHE_ITEMS = [
  { title: "晴天", variant: "【4K修复】Official MV", source: "bilibili", quality: "无损 FLAC", mb: 38.2, complete: true, played: "刚刚", pinned: true },
  { title: "七里香", variant: "周杰伦《七里香》完整版", source: "bilibili", quality: "无损 FLAC", mb: 42.6, complete: true, played: "2 小时前", pinned: false },
  { title: "稻香", variant: "【无损音质】稻香", source: "bilibili", quality: "无损 FLAC", mb: 31.8, complete: true, played: "昨天", pinned: false },
  { title: "青花瓷", variant: "官方 MV", source: "youtube", quality: "AAC 130K", mb: 4.2, complete: true, played: "昨天", pinned: false },
  { title: "告白气球", variant: "官方 MV", source: "youtube", quality: "AAC 130K", mb: 3.6, complete: true, played: "3 天前", pinned: false },
  { title: "夜曲", variant: "超高音质", source: "bilibili", quality: "192K AAC-LC", mb: 5.4, complete: true, played: "3 天前", pinned: false },
  { title: "Lemon", variant: "米津玄師 Official Video", source: "bilibili", quality: "无损 FLAC", mb: 41.1, complete: true, played: "上周", pinned: false },
  { title: "夜曲", variant: "Official Audio", source: "youtube", quality: "AAC 130K", mb: 3.8, complete: true, played: "上周", pinned: false },
  { title: "晴天（钢琴改编版）", variant: "【钢琴】演奏版", source: "bilibili", quality: "192K AAC-LC", mb: 7.1, complete: true, played: "上周", pinned: false },
  { title: "反方向的钟", variant: "现场版", source: "bilibili", quality: "192K AAC-LC", mb: 6.2, complete: true, played: "上周", pinned: false },
  { title: "周杰伦 经典歌曲合集 50 首", variant: "【Hi-Res】合集", source: "bilibili", quality: "无损 FLAC", mb: 181.9, complete: false, played: "刚才中断", pinned: false },
];

const CACHE_POLICY = {
  limitGb: 8,
  hitRate: 0.82,
  evictedThisWeek: 3,
  addedThisSession: 2,
  rules: [
    { id: "auto", label: "播放时自动缓存", detail: "边下边播，收完才算完整", on: true },
    { id: "prefetch", label: "预取下一首", detail: "队列里下一首提前落盘", on: true },
    { id: "lyrics", label: "缓存歌词与封面", detail: "离线也能显示", on: true },
    { id: "favorites", label: "只缓存已收藏", detail: "省空间，但搜索听到的会丢", on: false },
  ],
};

const SOURCE_PREFS = {
  defaultSource: "bilibili",
  scopes: { bilibili: "join", youtube: "join" },
  qualityOrder: ["无损 FLAC", "192K AAC-LC", "132K AAC-LC"],
};

// ---------------------------------------------------------------------------
// Lyrics (晴天) — same mock timeline as the existing prototype, trimmed to fit.
// ---------------------------------------------------------------------------

const LYRICS_QINGTIAN = [
  { time: 19, text: "故事的小黄花" },
  { time: 23, text: "从出生那年就飘着" },
  { time: 27, text: "童年的荡秋千" },
  { time: 31, text: "随记忆一直晃到现在" },
  { time: 35, text: "Re So So Si Do Si La" },
  { time: 39, text: "So La Si Si Si Si La Si La So" },
  { time: 43, text: "吹着前奏望着天空" },
  { time: 47, text: "我想起花瓣试着掉落" },
  { time: 51, text: "为你翘课的那一天" },
  { time: 55, text: "花落的那一天" },
  { time: 58, text: "教室的那一间" },
  { time: 61, text: "我怎么看不见" },
  { time: 64, text: "消失的下雨天" },
  { time: 68, text: "我好想再淋一遍" },
  { time: 72, text: "没想到失去的勇气我还留着" },
  { time: 79, text: "好想再问一遍" },
  { time: 83, text: "你会等待还是离开" },
  { time: 90, text: "刮风这天 我试过握着你手" },
  { time: 96, text: "但偏偏 雨渐渐 大到我看你不见" },
];

const NOW = { entryId: "qingtian", elapsed: 72, buffered: 188 };

// ---------------------------------------------------------------------------
// What the connection slot promises. No third source is drawn — only the shape of
// the contract, so the "more sources later" claim is visible without inventing one.
// ---------------------------------------------------------------------------

const CONNECTOR = {
  label: "接入来源",
  contract: ["关键词搜索", "直链解析", "歌词 / 字幕", "账号或匿名会话"],
};

// ---------------------------------------------------------------------------
// Cover hue per recording. Kept low-saturation on purpose: mock cover art is a
// monogram placeholder, and a saturated thumbnail would compete with whatever
// source-identity system a direction builds. Shared so the same song keeps the
// same tint in all three directions.
// ---------------------------------------------------------------------------

const TRACK_HUES = {
  qingtian: 32,
  qilixiang: 96,
  daoxiang: 46,
  gaobaiqiqiu: 340,
  yeqv: 258,
  qinghuaci: 178,
  "qingtian-piano": 210,
  "jay-collection": 12,
};

const trackHue = (entry) => TRACK_HUES[entry.id] ?? 210;

// ---------------------------------------------------------------------------
// The comparison's control. `?merge=off` opens every direction in the expanded
// state so the two representations can be compared side by side (and linked to).
// ---------------------------------------------------------------------------

const initialMerged = () => new URLSearchParams(location.search).get("merge") !== "off";


// ---------------------------------------------------------------------------
// THE LIBRARY — the thing this app actually is.
//
// Sources are plumbing now. A song is a song: title, artist, album, and where it
// sits in your library. Which platforms hold it, at what quality, and whether one
// of them has died is real information — but it belongs behind a "来源" button,
// not in the row you scan while deciding what to listen to.
//
// Two states exist here that a video-entry browser never has to face:
//   * 已回退  — the preferred edition is gone, so another platform's copy plays
//   * 无法播放 — every edition is gone, and the library says so instead of hiding it
// ---------------------------------------------------------------------------

const LIBRARY = [
  {
    id: "qingtian", title: "晴天", artist: "周杰伦", album: "叶惠美", year: 2003,
    duration: 269, initial: "晴", hue: 32, addedLabel: "8 月 14 日",
    playlists: ["深夜循环", "华语现场"], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "【4K修复】Official MV", mb: 38.2, downloaded: true, ok: true },
      { source: "youtube", quality: "AAC 130K", note: "官方 MV", mb: 4.4, downloaded: false, ok: true },
    ],
  },
  {
    id: "qilixiang", title: "七里香", artist: "周杰伦", album: "七里香", year: 2004,
    duration: 299, initial: "七", hue: 96, addedLabel: "8 月 14 日",
    playlists: ["深夜循环"], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "完整版", mb: 42.6, downloaded: true, ok: true },
      { source: "youtube", quality: "AAC 130K", note: "官方 MV", mb: 4.9, downloaded: false, ok: true },
    ],
  },
  {
    id: "daoxiang", title: "稻香", artist: "周杰伦", album: "魔杰座", year: 2008,
    duration: 223, initial: "稻", hue: 46, addedLabel: "9 月 2 日",
    playlists: ["华语现场"], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "［无损音质］", mb: 31.8, downloaded: true, ok: true },
    ],
  },
  {
    id: "gaobaiqiqiu", title: "告白气球", artist: "周杰伦", album: "周杰伦的床边故事", year: 2016,
    duration: 215, initial: "告", hue: 340, addedLabel: "9 月 2 日",
    playlists: [], prefer: "youtube",
    editions: [
      { source: "youtube", quality: "AAC 130K", note: "官方 MV", mb: 3.6, downloaded: false, ok: true },
    ],
  },
  {
    id: "yeqv", title: "夜曲", artist: "周杰伦", album: "十一月的萧邦", year: 2005,
    duration: 227, initial: "夜", hue: 258, addedLabel: "7 月 21 日",
    playlists: ["深夜循环"], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "超高音质", mb: 0, downloaded: false, ok: false,
        gone: "条目已被 UP 主删除" },
      { source: "youtube", quality: "AAC 130K", note: "Official Audio", mb: 3.8, downloaded: false, ok: true },
    ],
  },
  {
    id: "qinghuaci", title: "青花瓷", artist: "周杰伦", album: "我很忙", year: 2007,
    duration: 239, initial: "青", hue: 178, addedLabel: "7 月 21 日",
    playlists: ["华语现场"], prefer: "youtube",
    editions: [
      { source: "youtube", quality: "AAC 130K", note: "官方 MV", mb: 4.2, downloaded: true, ok: true },
    ],
  },
  {
    id: "qingtian-piano", title: "晴天（钢琴改编版）", artist: "Animenz Piano", album: "Animenz 编曲集", year: 2019,
    duration: 312, initial: "琴", hue: 210, addedLabel: "10 月 1 日",
    playlists: [], prefer: "bilibili", noLyrics: true,
    editions: [
      { source: "bilibili", quality: "192K AAC-LC", note: "［钢琴］演奏版", mb: 7.1, downloaded: false, ok: true },
    ],
  },
  {
    id: "lemon", title: "Lemon", artist: "米津玄師", album: "STRAY SHEEP", year: 2018,
    duration: 256, initial: "L", hue: 8, addedLabel: "6 月 11 日",
    playlists: ["深夜循环"], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "Official Video", mb: 41.1, downloaded: true, ok: true },
      { source: "youtube", quality: "AAC 130K", note: "Official Video", mb: 4.1, downloaded: false, ok: true },
    ],
  },
  {
    id: "fanzou", title: "反方向的钟", artist: "周杰伦", album: "范特西", year: 2001,
    duration: 262, initial: "反", hue: 12, addedLabel: "5 月 30 日",
    playlists: [], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "192K AAC-LC", note: "现场版", mb: 6.2, downloaded: false, ok: true },
    ],
  },
  {
    id: "jay-collection", title: "周杰伦 经典歌曲合集 50 首", artist: "华语音乐搬运工", album: null, year: 2022,
    duration: 4532, initial: "合", hue: 20, addedLabel: "6 月 11 日",
    playlists: [], prefer: "bilibili", noLyrics: true, partial: true,
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "［Hi-Res］合集", mb: 181.9, downloaded: false, ok: true },
    ],
  },
  {
    id: "dongfengpo", title: "东风破", artist: "周杰伦", album: "叶惠美", year: 2003,
    duration: 315, initial: "东", hue: 285, addedLabel: "4 月 18 日",
    playlists: ["华语现场"], prefer: "bilibili",
    editions: [
      { source: "bilibili", quality: "无损 FLAC", note: "官方 MV", mb: 0, downloaded: false, ok: false,
        gone: "该视频已被版权方下架" },
    ],
  },
];

/** The edition that would play: the preferred one when it still works. */
const pickedEdition = (song) => {
  const preferred = song.editions.find((e) => e.source === song.prefer);
  if (preferred && preferred.ok) return preferred;
  return song.editions.find((e) => e.ok) || null;
};

const isPlayable = (song) => pickedEdition(song) !== null;
const isFallback = (song) => {
  const picked = pickedEdition(song);
  return !!picked && picked.source !== song.prefer;
};
const isDownloaded = (song) => song.editions.some((e) => e.ok && e.downloaded);

// ---------------------------------------------------------------------------
// Playlists. Half of them are somebody else's collection, which is the whole
// point: a platform's favourite folder can only ever be read, so anything you
// star inside this app has to land somewhere you own.
// ---------------------------------------------------------------------------

// A platform playlist can only hold songs that platform actually carries, so
// 默认收藏夹 has no 青花瓷 (YouTube-only) and 点赞的音乐 has no 稻香 (Bilibili-only).
// The local playlist can hold anything, which is exactly why it exists.
const PLAYLISTS = [
  { id: "late-night", name: "深夜循环", origin: "local", count: 24, secs: 6214, updated: "今天", writable: true,
    songs: ["qingtian", "qilixiang", "yeqv", "lemon", "qinghuaci", "daoxiang"] },
  { id: "default-fav", name: "默认收藏夹", origin: "bilibili", count: 128, secs: 32841, updated: "2 小时前", writable: false,
    songs: ["qingtian", "qilixiang", "daoxiang", "yeqv", "lemon", "fanzou", "dongfengpo"] },
  { id: "live", name: "华语现场", origin: "bilibili", count: 57, secs: 14208, updated: "昨天", writable: false,
    songs: ["daoxiang", "dongfengpo", "fanzou", "qingtian-piano", "jay-collection"] },
  { id: "yt-liked", name: "点赞的音乐", origin: "youtube", count: 41, secs: 9867, updated: "3 天前", writable: false,
    songs: ["qingtian", "qilixiang", "gaobaiqiqiu", "yeqv", "qinghuaci"] },
];

// ---------------------------------------------------------------------------
// What each source contributes to the library, and whether it can be written to.
// "只读同步" is not a limitation to apologise for — it is the correct relationship
// with somebody else's collection, and it is why the local library has to exist.
// ---------------------------------------------------------------------------

const LIBRARY_SOURCES = [
  {
    id: "local",
    kind: "本地收藏与歌单",
    mode: "可写",
    writable: true,
    on: true,
    brought: "18 个歌单 · 214 首",
    detail: "在 app 里收藏、建歌单、改顺序，都写在这里",
  },
  {
    id: "bilibili",
    kind: "收藏夹 · 观看历史",
    mode: "只读同步",
    writable: false,
    on: true,
    brought: "3 个收藏夹 · 186 首",
    detail: "只能读回来合并显示；要改动得去 B 站自己的收藏夹里改",
  },
  {
    id: "youtube",
    kind: "点赞与播放列表",
    mode: "只读同步",
    writable: false,
    on: true,
    brought: "2 个播放列表 · 41 首",
    detail: "同上。本版不申请写入权限",
  },
];

const SYNC = {
  lastAt: "今天 20:41",
  added: 6,
  removed: 1,
  merged: 23,
  notes: [
    "新增 6 首，移除 1 首（B 站那边取消收藏了）",
    "23 首在两家都有，库里依然只占一条",
    "1 首在 B 站已失效，已自动改用 YouTube 版本接上",
  ],
};

// ---------------------------------------------------------------------------
// Formatting helpers (shared by all three directions).
// ---------------------------------------------------------------------------

const fmtTime = (seconds) => {
  const s = Math.max(0, Math.floor(seconds || 0));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

const fmtLong = (seconds) => {
  const s = Math.max(0, Math.floor(seconds || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h ? `${h}:${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}` : `${m}:${String(s % 60).padStart(2, "0")}`;
};

const fmtMb = (mb) => (mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`);

const sourceOf = (id) => SOURCES[id];

Object.assign(window, {
  SOURCES,
  SOURCE_ORDER,
  QUERY,
  ENTRIES,
  QUEUE,
  CACHE,
  NOW,
  LYRICS_QINGTIAN,
  CONNECTOR,
  CAPS_BILIBILI,
  CAPS_YOUTUBE,
  CAP_GROUPS,
  CACHE_ITEMS,
  CACHE_POLICY,
  SOURCE_PREFS,
  LIBRARY,
  PLAYLISTS,
  LIBRARY_SOURCES,
  SYNC,
  pickedEdition,
  isPlayable,
  isFallback,
  isDownloaded,
  TRACK_HUES,
  trackHue,
  initialMerged,
  flatRows,
  mergedGroupCount,
  editionCount,
  bestEdition,
  fmtTime,
  fmtLong,
  fmtMb,
  sourceOf,
});

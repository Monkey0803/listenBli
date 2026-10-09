// data.jsx — mock content for the listenBli prototype.
//
// Everything here mirrors the shapes in src/api/models.rs so the prototype
// reads like the real product: Track { bvid, title, author, duration, cover },
// AudioQuality, FavFolder, and lyrics lines with optional translation.
// Cover art is a CSS monogram placeholder (`initial` + `h1/h2` hues) — wire it
// to `Track.cover` when real artwork is available.

const QUALITY = {
  flac: { id: "flac", label: "无损 FLAC", short: "FLAC", tone: "gold" },
  k192: { id: "k192", label: "192K AAC-LC", short: "192K", tone: "blue" },
  k132: { id: "k132", label: "132K", short: "132K", tone: "dim" },
  k64: { id: "k64", label: "64K (低清)", short: "64K", tone: "warn" },
};

// -- search datasets --------------------------------------------------------

const JAY_TRACKS = [
  { bvid: "BV1Jt411k7Qd", title: "【4K修复】周杰伦 - 晴天 (Official MV)", author: "杰威尔音乐", duration: 269, initial: "晴", h1: 28, h2: 348, flac: true },
  { bvid: "BV1Zs4y1k7Xm", title: "晴天 - 周杰伦 官方MV", author: "JVR Music", duration: 269, initial: "晴", h1: 210, h2: 265, flac: false },
  { bvid: "BV1qP4y1t7Rn", title: "【Hi-Res】周杰伦 经典歌曲合集 50首", author: "华语音乐搬运工", duration: 4532, initial: "周", h1: 200, h2: 230, flac: true, noLyrics: true },
  { bvid: "BV1dd4y1c7Wp", title: "周杰伦《七里香》完整版", author: "杰威尔音乐", duration: 299, initial: "七", h1: 96, h2: 150, flac: true },
  { bvid: "BV1nL411x7Tg", title: "【无损音质】稻香 - 周杰伦", author: "华语音乐馆", duration: 223, initial: "稻", h1: 42, h2: 88, flac: true },
  { bvid: "BV1kY4y1P7Qa", title: "周杰伦 - 告白气球 (官方MV)", author: "杰威尔音乐", duration: 215, initial: "告", h1: 330, h2: 20, flac: false },
  { bvid: "BV1xT4y1Z7Hd", title: "【钢琴】晴天 - 纯音乐演奏版", author: "钢琴小妹Piano", duration: 312, initial: "琴", h1: 250, h2: 300, flac: false, noLyrics: true },
  { bvid: "BV1Qs4y1R7Jk", title: "夜曲 - 周杰伦（超高音质）", author: "音乐现场Live", duration: 227, initial: "夜", h1: 226, h2: 276, flac: true },
  { bvid: "BV1Fh4y1d7Bn", title: "周杰伦 - 青花瓷 官方MV", author: "杰威尔音乐", duration: 239, initial: "青", h1: 168, h2: 200, flac: true },
  { bvid: "BV1mP4y1S7Vc", title: "【合集】周杰伦电影主题曲精选", author: "影视音乐汇", duration: 1806, initial: "影", h1: 290, h2: 320, flac: false, noLyrics: true },
  { bvid: "BV1gL411B7Np", title: "周杰伦 - 反方向的钟 (现场版)", author: "演唱会实录", duration: 262, initial: "反", h1: 12, h2: 40, flac: false },
  { bvid: "BV1wY4y1M7Kt", title: "【大会员专享】周杰伦 无与伦比演唱会 修复版", author: "演唱会档案馆", duration: 269, initial: "无", h1: 300, h2: 350, flac: false, restricted: true },
];

const LEMON_TRACKS = [
  { bvid: "BV1Lt411s7Yw", title: "米津玄師 - Lemon (Official Video)", author: "米津玄師", duration: 256, initial: "L", h1: 44, h2: 74, flac: true },
  { bvid: "BV1eJ411k7Pz", title: "【中日字幕】Lemon / 米津玄師 - 电视剧《非自然死亡》主题曲", author: "日音字幕组", duration: 256, initial: "L", h1: 190, h2: 230, flac: false },
  { bvid: "BV1aP4y1b7Qm", title: "Lemon - 米津玄師 (钢琴改编版)", author: "Animenz Piano", duration: 274, initial: "♪", h1: 262, h2: 300, flac: false, noLyrics: true },
];

const SEARCH_SETS = [
  { match: ["周杰伦", "jay", "晴天", "七里香", "稻香"], tracks: JAY_TRACKS },
  { match: ["lemon", "米津", "玄师", "日文", "非自然"], tracks: LEMON_TRACKS },
];

// -- lyrics -----------------------------------------------------------------
// timings are mock but plausible; `time` is seconds.

const LYRICS_QINGTIAN = [
  { time: 0, text: "（前奏）" },
  { time: 19, text: "故事的小黄花", tr: "A little yellow flower from our story" },
  { time: 23, text: "从出生那年就飘着", tr: "Has been drifting since the year it was born" },
  { time: 27, text: "童年的荡秋千", tr: "The swing of my childhood" },
  { time: 31, text: "随记忆一直晃到现在", tr: "Still swaying with memory into this day" },
  { time: 35, text: "Re So So Si Do Si La" },
  { time: 39, text: "So La Si Si Si Si La Si La So" },
  { time: 43, text: "吹着前奏望着天空", tr: "Playing the prelude, gazing at the sky" },
  { time: 47, text: "我想起花瓣试着掉落", tr: "I recall a petal trying to fall" },
  { time: 51, text: "为你翘课的那一天", tr: "The day I skipped class for you" },
  { time: 55, text: "花落的那一天", tr: "The day the flowers fell" },
  { time: 58, text: "教室的那一间", tr: "That one classroom" },
  { time: 61, text: "我怎么看不见", tr: "Why can't I see it now" },
  { time: 64, text: "消失的下雨天", tr: "The rainy day that vanished" },
  { time: 68, text: "我好想再淋一遍", tr: "How I want to be soaked in it once more" },
  { time: 72, text: "没想到失去的勇气我还留着", tr: "I never thought I'd still keep the courage I lost" },
  { time: 79, text: "好想再问一遍", tr: "How I want to ask you again" },
  { time: 83, text: "你会等待还是离开", tr: "Will you wait, or will you leave" },
  { time: 90, text: "刮风这天 我试过握着你手", tr: "On this windy day, I tried to hold your hand" },
  { time: 96, text: "但偏偏 雨渐渐 大到我看你不见", tr: "But the rain grew so heavy I could no longer see you" },
  { time: 103, text: "还要多久 我才能在你身边", tr: "How much longer until I can be by your side" },
  { time: 110, text: "等到放晴的那天 也许我会比较好一点", tr: "Maybe when it clears up, I'll feel a little better" },
  { time: 118, text: "从前从前 有个人爱你很久", tr: "Once upon a time, someone loved you for a long while" },
  { time: 125, text: "但偏偏 风渐渐 把距离吹得好远", tr: "But the wind slowly blew the distance so far apart" },
  { time: 132, text: "好不容易 又能再多爱一天", tr: "With such effort, I get to love one more day" },
  { time: 139, text: "但故事的最后 你好像还是说了拜拜", tr: "But at the end of the story, you still seemed to say goodbye" },
  { time: 150, text: "（间奏）" },
  { time: 168, text: "刮风这天 我试过握着你手", tr: "On this windy day, I tried to hold your hand" },
  { time: 174, text: "但偏偏 雨渐渐 大到我看你不见", tr: "But the rain grew so heavy I could no longer see you" },
  { time: 181, text: "还要多久 我才能在你身边", tr: "How much longer until I can be by your side" },
  { time: 188, text: "等到放晴的那天 也许我会比较好一点", tr: "Maybe when it clears up, I'll feel a little better" },
  { time: 196, text: "从前从前 有个人爱你很久", tr: "Once upon a time, someone loved you for a long while" },
  { time: 203, text: "但偏偏 风渐渐 把距离吹得好远", tr: "But the wind slowly blew the distance so far apart" },
  { time: 210, text: "好不容易 又能再多爱一天", tr: "With such effort, I get to love one more day" },
  { time: 217, text: "但故事的最后 你好像还是说了拜拜", tr: "But at the end of the story, you still seemed to say goodbye" },
  { time: 228, text: "（尾奏）" },
];

const LYRICS_LEMON = [
  { time: 0, text: "（前奏）" },
  { time: 14, text: "夢ならばどれほどよかったでしょう", tr: "如果这是一场梦，那该有多好" },
  { time: 21, text: "未だにあなたのことを夢にみる", tr: "至今仍会在梦里见到你" },
  { time: 27, text: "忘れた物を取りに帰るように", tr: "就像回去取回遗忘的东西一样" },
  { time: 34, text: "古びた思い出の埃を払う", tr: "拂去陈旧回忆上的灰尘" },
  { time: 41, text: "戻らない幸せがあることを", tr: "有些幸福再也回不去了" },
  { time: 48, text: "最後にあなたが教えてくれた", tr: "这是最后你教会我的事" },
  { time: 55, text: "言えずに隠してた昏い過去も", tr: "连那些没说出口、藏在心里的灰暗过去" },
  { time: 62, text: "あなたがいなきゃ永遠に昏いまま", tr: "若没有你，将会永远黯淡下去" },
  { time: 70, text: "きっともうこれ以上 傷つくことなど", tr: "一定不会再有比这更深的伤害" },
  { time: 77, text: "ありはしないとわかっている", tr: "我心里其实很清楚" },
  { time: 84, text: "あの日の悲しみさえ あの日の苦しみさえ", tr: "就连那天的悲伤，那天的痛苦" },
  { time: 92, text: "そのすべてを愛してた あなたとともに", tr: "连同这一切我都深爱着，与你一起" },
  { time: 100, text: "胸に残り離れない 苦いレモンの匂い", tr: "留在心中挥之不去的，是柠檬苦涩的香气" },
  { time: 108, text: "雨が降り止むまでは帰れない", tr: "在雨停之前我无法回去" },
  { time: 115, text: "今でもあなたはわたしの光", tr: "直到现在，你仍是我的光" },
  { time: 126, text: "（間奏）" },
  { time: 140, text: "暗闇であなたの背をなぞった", tr: "在黑暗中描摹着你的背影" },
  { time: 147, text: "その輪郭を鮮明に覚えている", tr: "那轮廓我依然记得清晰" },
  { time: 154, text: "受け止めきれないものと出会うたび", tr: "每当遇见无法承受的事物" },
  { time: 161, text: "溢れてやまないのは涙だけ", tr: "满溢而出的就只有眼泪" },
  { time: 169, text: "何をしていたの 何を見ていたの", tr: "你在做什么，你在看着什么" },
  { time: 176, text: "わたしの知らない横顔で", tr: "用我所不知道的侧脸" },
  { time: 183, text: "どこかであなたが今 わたしと同じ様な", tr: "如果你此刻在某处，和我一样" },
  { time: 191, text: "涙にくれ 淋しさの中にいるなら", tr: "泪流满面，身处在寂寞之中" },
  { time: 199, text: "わたしのことなどどうか 忘れてください", tr: "请你把我忘了吧" },
  { time: 207, text: "そんなことを心から願うほどに", tr: "我发自内心如此期盼" },
  { time: 215, text: "今でもあなたはわたしの光", tr: "直到现在，你仍是我的光" },
  { time: 228, text: "（尾奏）" },
];

const LYRICS = {
  BV1Jt411k7Qd: { source: "bilibili-cc", lines: LYRICS_QINGTIAN },
  BV1Zs4y1k7Xm: { source: "netease", lines: LYRICS_QINGTIAN },
  BV1Lt411s7Yw: { source: "netease", lines: LYRICS_LEMON },
  BV1eJ411k7Pz: { source: "bilibili-cc", lines: LYRICS_LEMON },
  BV1dd4y1c7Wp: { source: "netease", lines: [
    { time: 0, text: "（前奏）" },
    { time: 16, text: "窗外的麻雀 在电线杆上多嘴", tr: "Sparrows outside chatter on the power line" },
    { time: 23, text: "妳说这一句 很有夏天的感觉", tr: "You say that line feels a lot like summer" },
    { time: 30, text: "手中的铅笔 在纸上来来回回", tr: "The pencil in my hand paces across the paper" },
    { time: 37, text: "我用几行字形容妳是我的谁", tr: "With a few lines I describe who you are to me" },
    { time: 44, text: "秋刀鱼的滋味 猫跟妳都想了解", tr: "The taste of saury — the cat and you both want to know" },
    { time: 51, text: "初恋的香味就这样被我们寻回", tr: "That way we rediscovered the scent of first love" },
    { time: 58, text: "那温暖的阳光 像刚摘的鲜艳草莓", tr: "That warm sunlight, like a freshly picked bright strawberry" },
    { time: 65, text: "妳说妳舍不得吃掉这一种感觉", tr: "You say you can't bear to eat away this feeling" },
    { time: 73, text: "雨下整夜 我的爱溢出就像雨水", tr: "Rain all night, my love overflows like the rain" },
    { time: 80, text: "院子落叶 跟我的思念厚厚一叠", tr: "Leaves in the yard pile as thick as my longing" },
    { time: 87, text: "几句是非 也无法将我的热情冷却", tr: "A few words of gossip can't cool my passion" },
    { time: 94, text: "妳出现在我诗的每一页", tr: "You appear on every page of my poem" },
  ] },
  BV1nL411x7Tg: { source: "netease", lines: [
    { time: 0, text: "（前奏）" },
    { time: 12, text: "对这个世界如果你有太多的抱怨", tr: "If you have too many complaints about this world" },
    { time: 19, text: "跌倒了就不敢继续往前走", tr: "And after falling you dare not keep walking" },
    { time: 26, text: "为什么人要这么的脆弱 堕落", tr: "Why must people be so fragile and fall" },
    { time: 33, text: "请你打开电视看看", tr: "Please turn on the TV and look" },
    { time: 40, text: "多少人为生命在努力勇敢的走下去", tr: "How many are bravely striving on for life" },
    { time: 47, text: "我们是不是该知足", tr: "Shouldn't we be content" },
    { time: 54, text: "珍惜一切 就算没有拥有", tr: "Cherish everything, even without owning it" },
    { time: 62, text: "还记得你说家是唯一的城堡", tr: "Remember you said home is the only castle" },
    { time: 69, text: "随着稻香河流继续奔跑", tr: "Keep running along the rice-scented river" },
    { time: 76, text: "微微笑 小时候的梦我知道", tr: "Smile a little — I know the childhood dream" },
    { time: 83, text: "不要哭 让萤火虫带着你逃跑", tr: "Don't cry, let the fireflies carry you away" },
  ] },
  BV1Qs4y1R7Jk: { source: "netease", lines: [
    { time: 0, text: "（前奏）" },
    { time: 18, text: "一群嗜血的蚂蚁 被腐肉所吸引", tr: "A swarm of bloodthirsty ants drawn by rotting flesh" },
    { time: 25, text: "我面无表情 看孤独的风景", tr: "Expressionless, I watch the lonely scenery" },
    { time: 32, text: "失去妳 爱恨开始分明", tr: "Losing you, love and hate begin to separate" },
    { time: 39, text: "失去妳 还有什么事好关心", tr: "Losing you, what else is worth caring about" },
    { time: 46, text: "当鸽子不再象征和平", tr: "When the dove no longer symbolizes peace" },
    { time: 53, text: "我终于被提醒 广场上喂食的是秃鹰", tr: "I'm finally reminded — it's vultures being fed in the square" },
    { time: 60, text: "我用漂亮的押韵 形容被掠夺一空的爱情", tr: "I use pretty rhymes to describe love plundered empty" },
  ] },
};

// -- library ----------------------------------------------------------------

const FAV_FOLDERS = [
  { id: 3017483, title: "默认收藏夹", media_count: 128 },
  { id: 3017484, title: "深夜循环", media_count: 24 },
  { id: 3017485, title: "华语现场", media_count: 57 },
  { id: 3017486, title: "待听清单", media_count: 9 },
];

const HISTORY_TRACKS = [
  { bvid: "BV1Qs4y1R7Jk", title: "夜曲 - 周杰伦（超高音质）", author: "音乐现场Live", duration: 227, initial: "夜", h1: 226, h2: 276, flac: true },
  { bvid: "BV1Fh4y1d7Bn", title: "周杰伦 - 青花瓷 官方MV", author: "杰威尔音乐", duration: 239, initial: "青", h1: 168, h2: 200, flac: true },
  { bvid: "BV1Lt411s7Yw", title: "米津玄師 - Lemon (Official Video)", author: "米津玄師", duration: 256, initial: "L", h1: 44, h2: 74, flac: true },
  { bvid: "BV1nL411x7Tg", title: "【无损音质】稻香 - 周杰伦", author: "华语音乐馆", duration: 223, initial: "稻", h1: 42, h2: 88, flac: true },
  { bvid: "BV1aP4y1b7Qm", title: "Lemon - 米津玄師 (钢琴改编版)", author: "Animenz Piano", duration: 274, initial: "♪", h1: 262, h2: 300, flac: false, noLyrics: true },
  { bvid: "BV1kY4y1P7Qa", title: "周杰伦 - 告白气球 (官方MV)", author: "杰威尔音乐", duration: 215, initial: "告", h1: 330, h2: 20, flac: false },
];

const USER = { mid: 88231147, uname: "夜航西飞", vip: true };

// -- helpers ----------------------------------------------------------------

const fmtTime = (seconds) => {
  const s = Math.max(0, Math.floor(seconds || 0));
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
};

const fmtBytes = (bytes) => {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
};

const fmtCount = (n) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);

const sourceLabel = (source) =>
  ({
    "bilibili-cc": "B 站 CC 字幕",
    netease: "网易云音乐",
    none: "无",
  })[source] || "无";

const searchTracks = (keyword) => {
  const q = (keyword || "").trim().toLowerCase();
  if (!q) return null;
  const set = SEARCH_SETS.find((entry) =>
    entry.match.some((token) => q.includes(token.toLowerCase()))
  );
  return set ? set.tracks : [];
};

Object.assign(window, {
  QUALITY,
  JAY_TRACKS,
  LEMON_TRACKS,
  LYRICS,
  FAV_FOLDERS,
  HISTORY_TRACKS,
  USER,
  fmtTime,
  fmtBytes,
  fmtCount,
  sourceLabel,
  searchTracks,
});

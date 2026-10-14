// library-pages.jsx — the 听歌 re-cut.
//
// The centre is 我的歌 and 正在播放. A song is a song: one row component serves
// 我的歌 / 最近播放 / 已下载 / 搜索结果, and none of them carry a platform. Which
// platform holds it, at what bitrate, and whether the preferred copy died is
// real information — it lives behind the 「来源」 button, in LSourceSheet.

// ---------------------------------------------------------------------------
// Shared: what state is this song in
// ---------------------------------------------------------------------------

/**
 * The one state worth showing in a list. Precedence is about what the listener
 * needs to know *before* pressing play: can it play at all, is it playing the
 * copy I meant, is all of it on disk.
 */
const lState = (song) => {
  if (!isPlayable(song)) return { led: "is-warn", label: "全部失效" };
  if (isFallback(song)) return { led: "is-warn", label: "已回退" };
  if (song.partial) return { led: "is-warn", label: "仅前半" };
  if (isDownloaded(song)) return { led: "is-ok", label: "已下载" };
  return { led: "is-idle", label: "在线" };
};

const lCoverBg = (song) => `linear-gradient(142deg, hsl(${song.hue} 15% 25%), hsl(${song.hue} 20% 12%))`;

function LSongRow({ song, index, now, meta, action, onPlay }) {
  const dead = !isPlayable(song);
  const st = lState(song);
  return (
    <div
      className={`lsong__row ${now ? "is-now" : ""} ${dead ? "is-gone" : ""}`}
      role="button"
      tabIndex={dead ? -1 : 0}
      title={dead ? `${song.title} · 所有来源都已失效` : `播放 ${song.title}`}
    >
      <span className="lsong__no">{String(index + 1).padStart(2, "0")}</span>

      <span className="a-cover a-cover--sm" style={{ background: lCoverBg(song) }}>
        {now ? (
          <span className="a-cover__eq">
            <Icon name="wave" size={11} />
          </span>
        ) : (
          song.initial
        )}
      </span>

      <div className="a-title">
        <div className="a-title__t" style={{ fontSize: 13.5 }}>
          {song.title}
        </div>
        <div className="a-title__s" style={{ fontSize: 11 }}>
          {song.artist}
        </div>
      </div>

      <span className="lsong__album">{song.album ? song.album : <i>—</i>}</span>
      <span className="a-dur">{fmtTime(song.duration)}</span>

      <span className="lsong__state">
        <span className={`a-led ${st.led}`} />
        <span>{st.label}</span>
      </span>

      <span className="lsong__added">{meta}</span>

      {action ? (
        action
      ) : (
        <button className="lsong__more" title="更多操作">
          <Icon name="more" size={14} />
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 库导航
// ---------------------------------------------------------------------------

const L_NAV = [
  { id: "library", label: "我的歌", icon: "lyrics", n: LIBRARY.length === 11 ? 214 : LIBRARY.length },
  { id: "playlists", label: "歌单", icon: "folder", n: PLAYLISTS.length === 4 ? 18 : PLAYLISTS.length },
  { id: "recent", label: "最近播放", icon: "clock", n: 40 },
  { id: "offline", label: "已下载", icon: "download", n: 8 },
];

function LNav({ page, onPage, onSources, onConnect }) {
  return (
    <aside className="a-rail lnav">
      <div className="a-rail__head">
        <span className="a-eng">My library</span>
        <span className="a-eng">3 处收藏</span>
      </div>

      <div className="lnav__group">
        {L_NAV.map((it) => (
          <button
            key={it.id}
            className={`lnav__item ${page === it.id ? "is-on" : ""}`}
            onClick={() => onPage(it.id)}
          >
            <span className="lnav__ic">
              <Icon name={it.icon} size={13} />
            </span>
            <span className="lnav__t">{it.label}</span>
            <span className="lnav__n">{it.n}</span>
          </button>
        ))}
      </div>

      <div className="lnav__group">
        <button className={`lnav__item ${page === "search" ? "is-on" : ""}`} onClick={() => onPage("search")}>
          <span className="lnav__ic">
            <Icon name="search" size={13} />
          </span>
          <span className="lnav__t">搜索</span>
          <span className="lnav__n">⌘K</span>
        </button>
      </div>

      <div className="lnav__group">
        <button className={`lnav__item ${page === "sources" ? "is-on" : ""}`} onClick={onSources}>
          <span className="lnav__ic">
            <Icon name="plug" size={13} />
          </span>
          <span className="lnav__t">来源与同步</span>
          <span className="lnav__n">{LIBRARY_SOURCES.length}</span>
        </button>
      </div>

      <div className="lnav__foot">
        <div className="lnav__sync">
          <span className="a-led is-ok" />
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg3)" }}>
            上次同步 {SYNC.lastAt}
          </span>
        </div>
        <button className="lnav__add" onClick={onConnect}>
          <Icon name="plus" size={13} />
          <span>{CONNECTOR.label}</span>
        </button>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// 我的歌 — the landing page
// ---------------------------------------------------------------------------

const L_FILTERS = [
  { id: "all", label: "全部" },
  { id: "down", label: "已下载" },
  { id: "stream", label: "未下载" },
  { id: "issue", label: "有问题的" },
];

function LLibraryPage({ current, setCurrent, onSource }) {
  const [filter, setFilter] = React.useState("all");
  const rows = LIBRARY.filter((s) => {
    if (filter === "down") return isDownloaded(s);
    if (filter === "stream") return !isDownloaded(s) && isPlayable(s);
    if (filter === "issue") return !isPlayable(s) || isFallback(s) || s.partial;
    return true;
  });
  const issues = LIBRARY.filter((s) => !isPlayable(s) || isFallback(s) || s.partial).length;

  return (
    <main className="a-list">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>我的歌</h1>
            <span className="a-tag is-dim">跨 3 处收藏</span>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>214</b> 首
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              <b>18</b> 个歌单
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              已下载 <b>17</b> 首
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              {issues} 首需要留意
            </span>
          </div>
        </div>

        <div style={{ flex: "1 1 auto" }} />

        <div className="a-seg">
          {L_FILTERS.map((f) => (
            <button
              key={f.id}
              className={`a-seg__cell ${filter === f.id ? "is-on" : ""}`}
              onClick={() => setFilter(f.id)}
            >
              {f.label}
            </button>
          ))}
        </div>

        <button className="a-btn is-primary">
          <Icon name="play" size={11} />
          全部播放
        </button>
      </div>

      <div className="a-cols lsong__cols">
        <span className="a-eng">#</span>
        <span />
        <span className="a-eng">歌曲</span>
        <span className="a-eng">专辑</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          时长
        </span>
        <span className="a-eng">状态</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          加入
        </span>
        <span />
      </div>

      <div className="a-rows">
        {rows.map((song, i) => (
          <div key={song.id} onClick={() => isPlayable(song) && setCurrent(song.id)}>
            <LSongRow
              song={song}
              index={LIBRARY.indexOf(song)}
              now={song.id === current}
              meta={song.addedLabel}
            />
          </div>
        ))}
        {rows.length === 0 ? (
          <div className="a-note" style={{ marginTop: 10 }}>
            <Icon name="filter" size={14} style={{ color: "var(--a-fg4)", flex: "none" }} />
            <span className="a-note__t">这个筛选下没有歌。</span>
          </div>
        ) : null}
      </div>

      <div className="a-queue">
        <div className="a-queue__head">
          <span className="a-eng">Library sync · {SYNC.lastAt}</span>
          <button className="a-eng" style={{ color: "var(--a-fg2)" }} onClick={onSource}>
            同步详情 ▸
          </button>
        </div>
        <div style={{ display: "flex", gap: 18, marginTop: 8 }}>
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            新增 <b style={{ color: "var(--a-green)", fontWeight: 500 }}>{SYNC.added}</b>
          </span>
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            移除 <b style={{ color: "var(--a-fg2)", fontWeight: 500 }}>{SYNC.removed}</b>
          </span>
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            两家都有的合并了 <b style={{ color: "var(--a-amber)", fontWeight: 500 }}>{SYNC.merged}</b> 首
          </span>
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            失效已回退 <b style={{ color: "var(--a-amber)", fontWeight: 500 }}>1</b> 首
          </span>
        </div>
      </div>
    </main>
  );
}

// ---------------------------------------------------------------------------
// 最近播放
// ---------------------------------------------------------------------------

const RECENT = [
  { day: "今天", items: [["qingtian", "20:41"], ["yeqv", "20:08"], ["lemon", "19:52"]] },
  { day: "昨天", items: [["qilixiang", "23:10"], ["qinghuaci", "22:31"], ["daoxiang", "09:14"]] },
  { day: "更早", items: [["gaobaiqiqiu", "周一 21:03"], ["fanzou", "上周"], ["dongfengpo", "上周"]] },
];

function LRecentPage({ current, setCurrent }) {
  return (
    <main className="a-list">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>最近播放</h1>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>40</b> 首
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">跨 3 处收藏</span>
            <span className="a-list__sep" />
            <span className="a-list__m">按播放时间</span>
          </div>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        <button className="a-btn">清空记录</button>
      </div>

      <div className="a-cols lsong__cols">
        <span className="a-eng">#</span>
        <span />
        <span className="a-eng">歌曲</span>
        <span className="a-eng">专辑</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          时长
        </span>
        <span className="a-eng">状态</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          播放于
        </span>
        <span />
      </div>

      <div className="a-rows">
        {RECENT.map((group) => (
          <React.Fragment key={group.day}>
            <div className="lsong__day">
              <b>{group.day}</b>
              <span>{group.items.length} 首</span>
            </div>
            {group.items.map(([id, at]) => {
              const song = LIBRARY.find((s) => s.id === id);
              if (!song) return null;
              return (
                <div key={id} onClick={() => isPlayable(song) && setCurrent(song.id)}>
                  <LSongRow song={song} index={LIBRARY.indexOf(song)} now={song.id === current} meta={at} />
                </div>
              );
            })}
          </React.Fragment>
        ))}
      </div>
    </main>
  );
}

// ---------------------------------------------------------------------------
// 歌单 — and the read/write question, which is the real subject here
// ---------------------------------------------------------------------------

function LPlaylistPage({ open, setOpen, current, setCurrent }) {
  const pl = PLAYLISTS.find((p) => p.id === open);
  const songs = pl ? pl.songs.map((id) => LIBRARY.find((s) => s.id === id)).filter(Boolean) : [];

  if (!pl) {
    return (
      <main className="a-list">
        <div className="a-list__head">
          <div>
            <div className="a-list__q">
              <h1>歌单</h1>
            </div>
            <div className="a-list__meta">
              <span className="a-list__m">
                <b>{PLAYLISTS.length}</b> 个
              </span>
              <span className="a-list__sep" />
              <span className="a-list__m">
                其中 <b>1</b> 个可以改
              </span>
              <span className="a-list__sep" />
              <span className="a-list__m">另外 3 个是别处的收藏，只读</span>
            </div>
          </div>
          <div style={{ flex: "1 1 auto" }} />
          <button className="a-btn is-primary">
            <Icon name="plus" size={11} />
            新建歌单
          </button>
        </div>

        <div className="a-cols lpl__cols">
          <span />
          <span className="a-eng">歌单</span>
          <span className="a-eng">来自</span>
          <span className="a-eng" style={{ textAlign: "right" }}>
            曲目
          </span>
          <span className="a-eng">读写</span>
          <span className="a-eng" style={{ textAlign: "right" }}>
            更新于
          </span>
          <span />
        </div>

        <div className="a-rows">
          {PLAYLISTS.map((p) => {
            const s = SOURCES[p.origin];
            return (
              <div className="lpl__row" key={p.id} role="button" tabIndex={0} onClick={() => setOpen(p.id)}>
                <span
                  className="lpl__cover"
                  style={{
                    background: p.writable
                      ? "var(--a-amber-dim)"
                      : "linear-gradient(142deg, #232329, #14141a)",
                  }}
                >
                  <Icon name={p.writable ? "folder" : "lock"} size={14} />
                </span>

                <div className="a-title">
                  <div className="a-title__t" style={{ fontSize: 13.5 }}>
                    {p.name}
                  </div>
                  <div className="a-title__s" style={{ fontSize: 11 }}>
                    {p.writable ? "在这个 app 里可以增删改" : "只能读回来，改动要回原处"}
                  </div>
                </div>

                <span className="lpl__from">
                  {s ? (
                    <span className="a-tag is-dim">{s.short}</span>
                  ) : (
                    <span className="a-tag" style={{ color: "var(--a-amber)", borderColor: "rgba(255,180,84,.35)" }}>
                      本地
                    </span>
                  )}
                </span>

                <span className="a-dur">{p.count} 首</span>
                <span className="lpl__mode">{p.writable ? "可写" : "只读同步"}</span>
                <span className="lpl__updated">{p.updated}</span>
                <span className="lsong__more">
                  <Icon name="chevronRight" size={14} />
                </span>
              </div>
            );
          })}
        </div>
      </main>
    );
  }

  const src = SOURCES[pl.origin];
  return (
    <main className="a-list">
      <div className="a-list__head">
        <div className="lpl__title">
          <span className="lpl__bigcover" style={{ background: pl.writable ? "var(--a-amber-dim)" : "linear-gradient(142deg, #232329, #14141a)" }}>
            <Icon name={pl.writable ? "folder" : "lock"} size={20} />
          </span>
          <div>
            <h1>{pl.name}</h1>
            <div className="lpl__totals">
              {src ? <span className="a-tag is-dim">{src.short} 收藏夹</span> : <span className="a-tag">本地歌单</span>}
              <span>{pl.count} 首</span>
              <span>约 {fmtLong(pl.secs)}</span>
              <span>更新于 {pl.updated}</span>
            </div>
          </div>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        <button className="a-btn">
          <Icon name="download" size={11} />
          全部下载
        </button>
        <button className="a-btn is-primary">
          <Icon name="play" size={11} />
          播放
        </button>
        <button className="a-btn" onClick={() => setOpen(null)}>
          返回
        </button>
      </div>

      {!pl.writable ? (
        <div className="lpl__banner">
          <Icon name="lock" size={14} style={{ color: "var(--a-amber)", flex: "none" }} />
          <span>
            这是 <b>{SOURCES[pl.origin].name}</b> 上的收藏夹，在这里只能读。
            你在 app 里对某一首歌点收藏，会写进<b>本地收藏</b>；想让改动回到 {SOURCES[pl.origin].name}，
            要去它自己的收藏夹里改。
            <span className="a-mono" style={{ display: "inline-block", marginLeft: 8, fontSize: 10 }}>
              需要的话可以「复制为本地歌单」，复制后就能自由增删
            </span>
          </span>
        </div>
      ) : null}

      <div className="a-cols lsong__cols">
        <span className="a-eng">#</span>
        <span />
        <span className="a-eng">歌曲</span>
        <span className="a-eng">专辑</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          时长
        </span>
        <span className="a-eng">状态</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          加入
        </span>
        <span />
      </div>

      <div className="a-rows">
        {songs.map((song) => (
          <div key={song.id} onClick={() => isPlayable(song) && setCurrent(song.id)}>
            <LSongRow song={song} index={songs.indexOf(song)} now={song.id === current} meta={song.addedLabel} />
          </div>
        ))}
      </div>

      <div className="a-queue">
        <div className="a-queue__head">
          <span className="a-eng">
            Showing {songs.length} of {pl.count}
          </span>
          <button className="a-eng" style={{ color: "var(--a-fg2)" }}>
            加载更多 ▸
          </button>
        </div>
      </div>
    </main>
  );
}

// ---------------------------------------------------------------------------
// 已下载 — grouped by the playlist you downloaded it for, which is how you
// actually consume offline music
// ---------------------------------------------------------------------------

const LOFF_GROUPS = [
  { name: "深夜循环", ids: ["qingtian", "qilixiang", "yeqv", "lemon"] },
  { name: "华语现场", ids: ["daoxiang", "qinghuaci", "dongfengpo"] },
  { name: "未归入歌单", ids: ["jay-collection"] },
];

function LOfflinePage({ current, setCurrent }) {
  const all = LOFF_GROUPS.flatMap((g) => g.ids);
  const onDisk = all.filter((id) => {
    const s = LIBRARY.find((x) => x.id === id);
    return s && (isDownloaded(s) || s.partial);
  });
  return (
    <main className="a-list">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>已下载</h1>
            <span className="a-tag is-dim">离线可听</span>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{onDisk.length}</b> 首在本地
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              共 <b>{fmtMb(CACHE.totalMb)}</b>
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              <b>1</b> 首只下到一半
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">上限 {fmtMb(CACHE.limitMb)}</span>
          </div>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        <span className="md__path" title={CACHE.folder}>
          <Icon name="folder" size={12} />
          <span>{CACHE.folder}</span>
        </span>
        <button className="a-btn">清理未固定</button>
      </div>

      <div className="loff__store">
        <div className="a-mtr">
          <div className="a-mtr__row">
            <span className="a-eng">音频</span>
            <span className="a-mtr__v is-amber">343.4 MB</span>
          </div>
          <div className="a-mtr__bar">
            <div className="a-mtr__fill" style={{ width: "80%" }} />
          </div>
        </div>
        <div className="a-mtr">
          <div className="a-mtr__row">
            <span className="a-eng">歌词与封面</span>
            <span className="a-mtr__v">12.8 MB</span>
          </div>
          <div className="a-mtr__bar">
            <div className="a-mtr__fill is-c" style={{ width: "3%" }} />
          </div>
        </div>
        <div className="a-mtr">
          <div className="a-mtr__row">
            <span className="a-eng">上限</span>
            <span className="a-mtr__v">{fmtMb(CACHE.limitMb)}</span>
          </div>
          <div className="a-mtr__bar">
            <div className="a-mtr__fill is-b" style={{ width: "5.2%" }} />
          </div>
        </div>
      </div>

      <div className="a-cols loff__cols">
        <span className="a-eng">#</span>
        <span />
        <span className="a-eng">歌曲</span>
        <span className="a-eng">专辑</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          大小
        </span>
        <span className="a-eng">状态</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          操作
        </span>
        <span />
      </div>

      <div className="a-rows">
        {LOFF_GROUPS.map((g) => (
          <React.Fragment key={g.name}>
            <div className="lsong__day">
              <b>{g.name}</b>
              <span>
                {g.ids.length} 首 · 已离线 {g.ids.filter((id) => {
                  const s = LIBRARY.find((x) => x.id === id);
                  return s && (isDownloaded(s) || s.partial);
                }).length}
              </span>
            </div>
            {g.ids.map((id, i) => {
              const song = LIBRARY.find((s) => s.id === id);
              if (!song) return null;
              const picked = pickedEdition(song);
              const size = song.editions.filter((e) => e.downloaded).reduce((n, e) => n + e.mb, 0) || song.editions[0].mb;
              const partial = !!song.partial;
              const done = isDownloaded(song);
              return (
                <div
                  className={`loff__row ${partial ? "is-partial" : ""}`}
                  key={id}
                  onClick={() => isPlayable(song) && setCurrent(song.id)}
                >
                  <span className="lsong__no">{String(i + 1).padStart(2, "0")}</span>
                  <span className="a-cover a-cover--sm" style={{ background: lCoverBg(song) }}>
                    {song.id === current ? (
                      <span className="a-cover__eq">
                        <Icon name="wave" size={11} />
                      </span>
                    ) : (
                      song.initial
                    )}
                  </span>
                  <div className="a-title">
                    <div className="a-title__t" style={{ fontSize: 13.5 }}>
                      {song.title}
                    </div>
                    <div className="a-title__s" style={{ fontSize: 11 }}>
                      {song.artist}
                    </div>
                  </div>
                  <span className="lsong__album">{song.album ? song.album : <i>—</i>}</span>
                  <span className="loff__mb">{done || partial ? `${size.toFixed(1)} MB` : "—"}</span>
                  <span className="lsong__state">
                    <span className={`a-led ${partial ? "is-warn" : done ? "is-ok" : "is-idle"}`} />
                    <span>{partial ? "仅前半" : done ? "完整" : "未下载"}</span>
                  </span>
                  <span className="loff__acts">
                    <button className="a-icobtn" title={done ? "固定，不参与淘汰" : "下载"}>
                      <Icon name={done ? "lock" : "download"} size={12} />
                    </button>
                    <button className="a-icobtn is-danger" title="删除本地副本">
                      <Icon name="close" size={12} />
                    </button>
                  </span>
                  <span />
                </div>
              );
            })}
          </React.Fragment>
        ))}
      </div>
    </main>
  );
}

// ---------------------------------------------------------------------------
// 搜索 — results are songs, and the primary action on one is 加入我的歌
// ---------------------------------------------------------------------------

const SEARCH_MORE = [
  {
    id: "huahai", title: "花海", artist: "周杰伦", album: "魔杰座", year: 2008, duration: 236,
    initial: "花", hue: 150, extra: true, prefer: "bilibili",
    editions: [{ source: "bilibili", quality: "192K AAC-LC", note: "现场版", mb: 6.1, downloaded: false, ok: true }],
  },
  {
    id: "guiji", title: "轨迹", artist: "周杰伦", album: "寻找周杰伦", year: 2003, duration: 265,
    initial: "轨", hue: 200, extra: true, prefer: "youtube",
    editions: [{ source: "youtube", quality: "AAC 130K", note: "官方 MV", mb: 4.3, downloaded: false, ok: true }],
  },
  {
    id: "anhao", title: "暗号", artist: "周杰伦", album: "八度空间", year: 2002, duration: 289,
    initial: "暗", hue: 270, extra: true, prefer: "bilibili",
    editions: [{ source: "bilibili", quality: "无损 FLAC", note: "［无损］", mb: 33.0, downloaded: false, ok: true }],
  },
];

function LSearchPage({ current, setCurrent }) {
  // 8 songs by him already in the library, plus 3 the search turned up that are not.
  const inLib = LIBRARY.filter((s) => s.artist === "周杰伦");
  const results = [...inLib, ...SEARCH_MORE];
  return (
    <main className="a-list">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>周杰伦</h1>
            <span className="a-tag is-dim">搜索</span>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{results.length}</b> 首
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              其中 <b>{inLib.length}</b> 首已在你的库里
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">跨 3 处来源 · 0.42s</span>
          </div>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        <button className="a-btn is-primary">
          <Icon name="plus" size={11} />
          全部加入我的歌
        </button>
      </div>

      <div className="a-cols lsong__cols">
        <span className="a-eng">#</span>
        <span />
        <span className="a-eng">歌曲</span>
        <span className="a-eng">专辑</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          时长
        </span>
        <span className="a-eng">状态</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          操作
        </span>
        <span />
      </div>

      <div className="a-rows">
        <div className="a-note">
          <Icon name="search" size={14} style={{ color: "var(--a-fg4)", flex: "none" }} />
          <span className="a-note__t">
            搜索结果和「我的歌」长得一样 —— 都是歌。已经在库里的没什么可做的，
            不在库里的那个动作才是<b>加入我的歌</b>，而不是「播放一下就算了」。
          </span>
        </div>

        {results.map((song, i) => (
          <div key={song.id} onClick={() => isPlayable(song) && setCurrent(song.id)}>
            <LSongRow
              song={song}
              index={i}
              now={song.id === current}
              meta={song.extra ? "" : "已在库"}
              action={
                song.extra ? (
                  <button className="a-btn" style={{ fontSize: 11, padding: "0 9px" }}>
                    <Icon name="plus" size={10} />
                    加入
                  </button>
                ) : (
                  <span className="lsong__more" style={{ opacity: 0.28 }}>
                    <Icon name="check" size={13} />
                  </span>
                )
              }
            />
          </div>
        ))}
      </div>
    </main>
  );
}

// ---------------------------------------------------------------------------
// 来源与同步 — where the platform went
// ---------------------------------------------------------------------------

function LSourcesPage({ onConnect }) {
  const [on, setOn] = React.useState({ local: true, bilibili: true, youtube: true });
  return (
    <main className="a-list">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>来源与同步</h1>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{LIBRARY_SOURCES.length}</b> 处收藏
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              合并为 <b>214</b> 首
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">上次同步 {SYNC.lastAt}</span>
          </div>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        <button className="a-btn">
          <Icon name="download" size={11} />
          立即同步
        </button>
        <button className="a-btn is-primary" onClick={onConnect}>
          <Icon name="plus" size={11} />
          接入来源
        </button>
      </div>

      <div className="a-rows">
        <div className="lsrc__sync">
          {[
            ["新增", SYNC.added, "is-green"],
            ["移除", SYNC.removed, ""],
            ["两家都有的合并", SYNC.merged, "is-amber"],
            ["同步失败", 0, "is-green"],
          ].map(([label, value, tone]) => (
            <div className="a-mtr" key={label}>
              <div className="a-mtr__row">
                <span className="a-eng">{label}</span>
                <span className={`a-mtr__v ${tone}`}>{value}</span>
              </div>
            </div>
          ))}
        </div>

        {SYNC.notes.map((n) => (
          <div className="lsrc__note" key={n}>
            <i>
              <span className="a-led is-ok" />
            </i>
            <span>{n}</span>
          </div>
        ))}

        {LIBRARY_SOURCES.map((s) => {
          const src = SOURCES[s.id];
          return (
            <div className="lsrc__module" key={s.id}>
              <div className="lsrc__head">
                <span className={`lsrc__badge ${s.writable ? "is-local" : ""}`}>
                  {src ? src.channel : <Icon name="folder" size={13} />}
                </span>
                <span className="lsrc__name">{src ? src.name : "本地收藏"}</span>
                <span className={`a-led ${on[s.id] ? "is-ok" : "is-idle"}`} />
                <span className="a-eng">{s.kind}</span>
                <div style={{ flex: "1 1 auto" }} />
                <span className={`a-tag ${s.writable ? "" : "is-dim"}`} style={s.writable ? { color: "var(--a-amber)", borderColor: "rgba(255,180,84,.35)" } : undefined}>
                  <Icon name={s.writable ? "folder" : "lock"} size={10} />
                  {s.mode}
                </span>
                <button className={`a-sw ${on[s.id] ? "is-on" : ""}`} onClick={() => setOn({ ...on, [s.id]: !on[s.id] })}>
                  <span className="a-sw__track">
                    <i />
                  </span>
                </button>
              </div>

              <div className="lsrc__body">
                <div>
                  <span className="a-eng">带来了什么</span>
                  <div className="lsrc__what" style={{ marginTop: 6 }}>
                    {s.brought}
                  </div>
                  <div className="lsrc__detail">{s.detail}</div>
                </div>
                <div className="lsrc__side">
                  <span className="a-eng">
                    {s.id === "bilibili"
                      ? `能力声明 ${CAPS_BILIBILI.filter((c) => c.on).length} / ${CAPS_BILIBILI.length}`
                      : s.id === "youtube"
                      ? `能力声明 ${CAPS_YOUTUBE.filter((c) => c.on).length} / ${CAPS_YOUTUBE.length}`
                      : "本地库 · 全部可写"}
                  </span>
                  {s.writable ? (
                    <div className="lsrc__detail" style={{ marginTop: 0 }}>
                      可读 <b style={{ color: "var(--a-green)" }}>✓</b> · 可写{" "}
                      <b style={{ color: "var(--a-green)" }}>✓</b>
                      <br />
                      在 app 里点收藏、建歌单写在这里
                    </div>
                  ) : (
                    <div className="lsrc__detail" style={{ marginTop: 0 }}>
                      可读 <b style={{ color: "var(--a-green)" }}>✓</b> · 可写{" "}
                      <b style={{ color: "var(--a-fg4)" }}>✗</b>
                      <br />
                      别人的收藏，只读；改动要回原处
                    </div>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </main>
  );
}

// ---------------------------------------------------------------------------
// 正在播放（右栏）
// ---------------------------------------------------------------------------

function LNowPlaying({ song, onSource }) {
  const picked = pickedEdition(song);
  const st = lState(song);
  const idx = LYRICS_QINGTIAN.findIndex((l) => l.time === NOW.elapsed);
  const at = idx === -1 ? 14 : idx;
  const WINDOW = 10;
  const start = Math.max(0, Math.min(LYRICS_QINGTIAN.length - WINDOW, at - 4));
  const lines = song.noLyrics ? [] : LYRICS_QINGTIAN.slice(start, start + WINDOW);

  return (
    <aside className="lnow">
      <div className="lnow__head">
        <span className="a-eng">Now playing</span>
        <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg4)" }}>
          偏移 −0.2s
        </span>
      </div>

      <div className="lnow__art">
        <span className="lnow__cover" style={{ background: lCoverBg(song) }}>
          {song.initial}
          <span className="lnow__eq">
            <Icon name="wave" size={34} />
          </span>
        </span>
        <div className="lnow__meta">
          <div className="lnow__title">{song.title}</div>
          <div className="lnow__sub">
            {song.artist}
            {song.album ? ` · ${song.album}` : ""}
            {song.year ? ` · ${song.year}` : ""}
          </div>
          <div className="lnow__facts">
            <span className="a-tag is-dim">
              <span className={`a-led ${st.led}`} />
              {st.label}
            </span>
            {picked ? <span className="a-tag">{picked.quality}</span> : null}
            <button className="a-tag" style={{ color: "var(--a-fg2)" }} onClick={onSource}>
              来源 ▸
            </button>
          </div>
        </div>
      </div>

      <div className="lnow__lyrics">
        {song.noLyrics ? (
          <div className="a-note" style={{ margin: "0 14px" }}>
            <Icon name="lyrics" size={13} style={{ color: "var(--a-fg4)", flex: "none" }} />
            <span className="a-note__t">
              这首歌没有找到歌词 —— 歌词来源是自动匹配的，找不到就如实空着。
            </span>
          </div>
        ) : (
          lines.map((line, j) => {
            const i = start + j;
            return (
              <div
                key={i}
                className={`a-line ${i === at ? "is-now" : i < at ? "is-past" : i - at === 1 ? "is-next" : ""}`}
              >
                {line.text}
              </div>
            );
          })
        )}
      </div>

      <div className="lnow__foot">
        <div className="a-readout">
          <span className="a-eng">歌词来源</span>
          <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg3)" }}>
            {song.noLyrics ? "无" : "B 站 CC 字幕 · 人工"}
          </span>
        </div>
        <div className="a-readout">
          <span className="a-eng">优先规则</span>
          <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg3)" }}>
            音质 › 已下载 › 可用性
          </span>
        </div>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// 播放条 — the same deck, minus the channel tag, plus a way into 来源
// ---------------------------------------------------------------------------

function LDeck({ song, onSource }) {
  const picked = pickedEdition(song);
  const pct = NOW.elapsed / song.duration;
  const bufPct = Math.max(pct, NOW.buffered / song.duration);
  return (
    <footer className="a-deck">
      <div className="a-now">
        <span className="a-now__cover" style={{ background: lCoverBg(song) }}>
          {song.initial}
        </span>
        <div className="a-now__t">
          <div className="a-now__row">
            <span className="a-now__title">{song.title}</span>
            {picked ? <span className="a-tag">{picked.quality}</span> : null}
            {isFallback(song) ? <span className="a-tag" style={{ color: "var(--a-red)", borderColor: "rgba(242,99,95,.4)" }}>已回退</span> : null}
          </div>
          <div className="a-now__s">
            {song.artist}
            {song.album ? ` · ${song.album}` : ""}
            <button
              onClick={onSource}
              className="a-eng"
              style={{ marginLeft: 10, color: "var(--a-fg4)", textDecoration: "underline" }}
            >
              来源
            </button>
          </div>
        </div>
      </div>

      <div className="a-transport">
        <div className="a-buttons">
          <button className="a-tbtn" title="上一首">
            <Icon name="prev" size={13} />
          </button>
          <button className="a-tbtn is-play" title="播放 / 暂停">
            <Icon name="pause" size={15} />
          </button>
          <button className="a-tbtn" title="下一首">
            <Icon name="next" size={13} />
          </button>
        </div>
        <div className="a-seek">
          <span className="a-seek__t">{fmtTime(NOW.elapsed)}</span>
          <div className="a-seek__bar">
            <div className="a-seek__buf" style={{ width: `${bufPct * 100}%` }} />
            <div className="a-seek__fill" style={{ width: `${pct * 100}%` }} />
            <div className="a-seek__knob" style={{ left: `${pct * 100}%` }} />
          </div>
          <span className="a-seek__t">{fmtTime(song.duration)}</span>
        </div>
      </div>

      <div className="a-deck__right">
        <div className="a-deck__box">
          <div className="a-buf__row">
            <span className="a-eng">Streaming</span>
            <span className="a-buf__t">
              {NOW.buffered} / {song.duration} s
            </span>
          </div>
          <div className="a-buf__bar">
            <div className={`a-buf__fill ${isDownloaded(song) ? "" : "is-partial"}`} style={{ width: `${bufPct * 100}%` }} />
          </div>
        </div>
        <div className="a-vol">
          <Icon name="volume" size={14} />
          <div className="a-vol__bar">
            <div className="a-vol__fill" style={{ width: "80%" }} />
          </div>
        </div>
        <button className="a-tbtn" title="播放队列">
          <Icon name="queue" size={14} />
        </button>
      </div>
    </footer>
  );
}

// ---------------------------------------------------------------------------
// 来源详情 — "sources are hidden" only works if you can always ask
// ---------------------------------------------------------------------------

function LSourceSheet({ song, onClose }) {
  const picked = pickedEdition(song);
  const others = song.editions.filter((e) => e !== picked);
  const dead = !picked;

  return (
    <div className="lsheet__scrim" onClick={onClose}>
      <div className="lsheet" onClick={(e) => e.stopPropagation()}>
        <div className="lsheet__head">
          <span className="lsheet__cover" style={{ background: lCoverBg(song) }}>
            {song.initial}
          </span>
          <div>
            <div style={{ fontSize: 15, color: "var(--a-fg)" }}>{song.title}</div>
            <div className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)", marginTop: 4 }}>
              {song.artist}
              {song.album ? ` · ${song.album}` : ""}
              {song.year ? ` · ${song.year}` : ""}
            </div>
          </div>
          <div style={{ flex: "1 1 auto" }} />
          <button className="a-iconbtn" onClick={onClose}>
            <Icon name="close" size={13} />
          </button>
        </div>

        <div className="lsheet__body">
          <div className="lsheet__block">
            <span className="a-eng">{dead ? "没有可播的版本" : "正在播放的这一版"}</span>
            {dead ? (
              <div className="lsheet__playing" style={{ borderColor: "rgba(242,99,95,.4)", borderLeftColor: "var(--a-red)", background: "rgba(242,99,95,.08)" }}>
                <Icon name="close" size={14} style={{ color: "var(--a-red)", flex: "none" }} />
                <div>
                  <div style={{ fontSize: 12.5, color: "var(--a-fg)" }}>
                    {song.editions.length} 个版本都已失效
                  </div>
                  <div className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)", marginTop: 4 }}>
                    {song.editions.map((e) => e.gone).filter(Boolean).join("；")}
                  </div>
                </div>
              </div>
            ) : (
              <div className="lsheet__playing">
                <span className="a-led is-on" />
                <div style={{ flex: "1 1 auto" }}>
                  <div style={{ fontSize: 12.5, color: "var(--a-fg)" }}>
                    {SOURCES[picked.source].name}
                    <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)", marginLeft: 9 }}>
                      {picked.note}
                    </span>
                  </div>
                  <div className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)", marginTop: 4 }}>
                    {picked.quality}
                    {picked.downloaded ? ` · 已下载 ${picked.mb.toFixed(1)} MB` : " · 在线播放"}
                  </div>
                </div>
                <button className="a-btn">打开原视频 ▸</button>
              </div>
            )}
          </div>

          <div className="lsheet__block">
            <span className="a-eng">为什么是这一版</span>
            <div className="lsheet__why">
              <Icon name="filter" size={13} style={{ color: "var(--a-fg4)", flex: "none", marginTop: 2 }} />
              <span>
                {dead ? (
                  <>
                    这首歌的所有版本都取不到了。库里<b>保留着它</b>而不是悄悄删掉 ——
                    如果你在别处重新找到，可以直接换上。
                  </>
                ) : isFallback(song) ? (
                  <>
                    本来首选的是 <b>{SOURCES[song.prefer].name}</b> 那一版，但它已经失效
                    （{song.editions.find((e) => e.source === song.prefer)?.gone || "取不到"}）。
                    库里有另一版可用，所以<b>自动换成了 {SOURCES[picked.source].name}</b>，没有让这首歌变成不能播。
                  </>
                ) : (
                  <>
                    按<b>音质 › 已下载 › 可用性</b>排序，这一版的质量最高
                    {picked.downloaded ? "，而且已经在本地" : ""}。
                    这个优先级可以在设置里改。
                  </>
                )}
              </span>
            </div>
          </div>

          <div className="lsheet__block">
            <span className="a-eng">这首歌的全部版本 · {song.editions.length}</span>
            {song.editions.map((e) => {
              const on = e === picked;
              return (
                <div className={`lsheet__opt ${on ? "is-on" : ""} ${e.ok ? "" : "is-dead"}`} key={e.source}>
                  <span className={`a-led ${on ? "is-on" : e.ok ? "is-ok" : "is-warn"}`} />
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontSize: 12, color: "var(--a-fg)" }}>{SOURCES[e.source].name}</div>
                    <div className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg4)", marginTop: 3 }}>
                      {e.gone ? e.gone : e.note}
                    </div>
                  </div>
                  <span className="lsheet__opt__q">{e.quality}</span>
                  <span className="lsheet__opt__mb">{e.ok ? `${e.mb.toFixed(1)} MB` : "—"}</span>
                  <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg3)" }}>
                    {e.downloaded ? "已下载" : e.ok ? "在线" : "失效"}
                  </span>
                  <span style={{ display: "flex", justifyContent: "flex-end" }}>
                    {on ? (
                      <span className="a-mono" style={{ fontSize: 10, color: "var(--a-amber)" }}>
                        正在用
                      </span>
                    ) : (
                      <button className="a-btn" style={{ height: 22, fontSize: 10.5, padding: "0 8px" }}>
                        {e.ok ? "改用这一版" : "换掉"}
                      </button>
                    )}
                  </span>
                </div>
              );
            })}
          </div>
        </div>

        <div className="lsheet__foot">
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            {others.length} 个其它版本
          </span>
          <div style={{ flex: "1 1 auto" }} />
          <button className="a-btn">固定使用这一版</button>
          <button className="a-btn" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, {
  LNav,
  LSongRow,
  LLibraryPage,
  LRecentPage,
  LPlaylistPage,
  LOfflinePage,
  LSearchPage,
  LSourcesPage,
  LNowPlaying,
  LDeck,
  LSourceSheet,
  lState,
  lCoverBg,
});

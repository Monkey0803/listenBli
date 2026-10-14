// direction-b.jsx — Direction B · THE INDEX
//
// An aggregated player read as a printed catalogue. Sources are collections;
// the aggregate page is the index. Duplicates get the catalogue's own answer:
// one entry with two provenance stamps, and a "参见" cross-reference when the
// list is expanded to one row per holding.
//
// Vermilion means "now playing" and nothing else.

const B_TOC = [
  { id: "all", label: "全部", n: ENTRIES.length, on: true },
  { id: "bilibili", label: "B 站", n: 6, sess: { who: "夜航西飞", what: "大会员" } },
  { id: "youtube", label: "YouTube", n: 5, sess: { who: "匿名访问", what: "visitorData 已取" } },
];

const B_SHORT = (q) => (/FLAC/i.test(q) ? "无损" : (q.match(/(\d+)\s*K/i) || [, q])[1] + "K");

const bCoverBg = (entry) => `hsl(${trackHue(entry)} 26% 86%)`;

function BCover({ entry, size = "md", playing }) {
  return (
    <span
      className={`b-cover ${size === "sm" ? "b-cover--sm" : ""}`}
      style={{ background: bCoverBg(entry) }}
      aria-hidden="true"
    >
      {playing ? <Icon name="wave" size={size === "sm" ? 12 : 15} className="b-cover__eq" /> : entry.initial}
    </span>
  );
}

function BMasthead() {
  return (
    <header className="b-mast">
      <div className="b-mast__mark">
        <span className="b-mast__word">
          listen<i>Bli</i>
        </span>
        <span className="b-mast__sub">Aggregated audio index</span>
      </div>

      <div className="b-search">
        <Icon name="search" size={13} />
        <input value={QUERY.keyword} readOnly aria-label="检索" />
        <span className="b-search__key">⌘K</span>
      </div>

      <div className="b-mast__right">
        <div className="b-mread">
          <span className="b-lbl">已下载 · 17 首</span>
          <span className="b-mread__v">{fmtMb(CACHE.totalMb)}</span>
        </div>
        <div className="b-mread">
          <span className="b-lbl">来源 · 会话</span>
          <span className="b-mread__v">2 / 2 在线</span>
        </div>
        <button className="b-mbtn">
          <Icon name="plug" size={12} />
          来源与会话
        </button>
        <button className="b-mbtn">
          <Icon name="settings" size={12} />
          设置
        </button>
      </div>
    </header>
  );
}

function BToc() {
  return (
    <nav className="b-toc">
      <div className="b-toc__head">
        <span className="b-toc__title">目次</span>
        <span className="b-lbl">Contents</span>
      </div>

      <div className="b-toc__group">
        {B_TOC.map((t) => (
          <React.Fragment key={t.id}>
            <button className={`b-toc__item ${t.on ? "is-on" : ""}`}>
              <span className="b-toc__label">{t.label}</span>
              <span className="b-toc__n">{t.n}</span>
            </button>
            {t.sess ? (
              <div className="b-toc__sess">
                <b>{t.sess.who}</b> · {t.sess.what}
              </div>
            ) : null}
          </React.Fragment>
        ))}
      </div>

      <div className="b-toc__group">
        <button className="b-toc__item">
          <span className="b-toc__label">队列</span>
          <span className="b-toc__n">{QUEUE.length}</span>
        </button>
        <button className="b-toc__item">
          <span className="b-toc__label">已下载</span>
          <span className="b-toc__n">{CACHE.tracks}</span>
        </button>
        <button className="b-toc__item">
          <span className="b-toc__label">磁盘占用</span>
          <span className="b-toc__n">{fmtMb(CACHE.totalMb)}</span>
        </button>
      </div>

      <button className="b-toc__add">
        <span className="b-toc__add-t">
          <Icon name="plus" size={13} />
          {CONNECTOR.label}
        </span>
        <span className="b-toc__add-c">{CONNECTOR.contract.join(" · ")}</span>
      </button>

      <div className="b-toc__colophon">
        共 <b>{editionCount()} 个来源条目</b>，合并为 <b>{ENTRIES.length} 条著录</b>。
        <br />
        缓存目录 <b>{CACHE.folder}</b>
      </div>
    </nav>
  );
}

function BStamps({ entry, best, flat }) {
  return (
    <div className="b-stamps">
      {entry.editions.map((e) => {
        const s = SOURCES[e.source];
        const isBest = !flat && e === best;
        return (
          <span key={e.vid} className={`b-stamp ${isBest ? "b-stamp--filled" : "b-stamp--muted"}`}>
            {s.stare}
            <em style={{ fontStyle: "normal", opacity: 0.75 }}>{B_SHORT(e.quality)}</em>
            {e.cache.done ? "●" : ""}
          </span>
        );
      })}
    </div>
  );
}

function BIndex({ merged, setMerged, currentId }) {
  // One row per holding when expanded; numbering runs over holdings, and a
  // repeated recording gets a "参见" back to the first holding's number.
  let n = 0;
  const flat = ENTRIES.flatMap((entry) => {
    const first = n + 1;
    return entry.editions.map((edition, i) => ({ entry, edition, no: ++n, refTo: i > 0 ? first : null }));
  });

  return (
    <main className="b-index">
      <div className="b-index__head">
        <div className="b-index__q">
          <h1>{QUERY.keyword}</h1>
          <span className="b-num">检索 · {QUERY.took.toFixed(2)}s</span>
        </div>
        <div className="b-index__meta">
          <span>
            <b>{ENTRIES.length}</b> 条著录
          </span>
          <span>
            <b>{mergedGroupCount()}</b> 组合并
          </span>
          <span>
            来源条目 <b>{editionCount()}</b>
          </span>
          <span>
            来源 <b>{QUERY.sources}</b> 个
          </span>
        </div>

        <div className="b-index__bar">
          <button className={`b-toggle ${merged ? "is-on" : ""}`} onClick={() => setMerged(!merged)}>
            <span className="b-toggle__box">
              <i />
            </span>
            <span className="b-toggle__t">合并重复</span>
          </button>
          <button className="b-btn is-solid">
            <Icon name="play" size={11} />
            全部播放
          </button>
          <div style={{ flex: "1 1 auto" }} />
          <span className="b-lbl">按编号 · 音质降序</span>
        </div>
      </div>

      <div className="b-cols">
        <span className="b-lbl">编号</span>
        <span />
        <span className="b-lbl">著录</span>
        <span className="b-lbl">出处</span>
        <span className="b-lbl">音质</span>
        <span className="b-lbl" style={{ textAlign: "right" }}>
          时长
        </span>
        <span />
      </div>

      <div className="b-rows">
        <div className="b-note">
          <span className="b-note__mk">
            <Icon name="merge" size={13} weight={1.7} />
          </span>
          <span>
            {merged ? (
              <>
                同一录音在两家各有一个条目时，本目录只列<b>一条著录</b>，出处栏并列两枚章。
                播放取<b>音质最高、已缓存优先</b>的那个。
              </>
            ) : (
              <>
                已按来源条目展开为 <b>{editionCount()} 行</b>；重复的录音以
                <span className="b-title__ref"> 参见 </span>回指首次著录的编号。
              </>
            )}
          </span>
        </div>

        {merged
          ? ENTRIES.map((entry, i) => (
              <div
                key={entry.id}
                className={`b-row b-row--merged ${entry.id === currentId ? "is-current" : ""}`}
                role="button"
                tabIndex={0}
              >
                <span className="b-row__no">{String(i + 1).padStart(2, "0")}</span>
                <BCover entry={entry} playing={entry.id === currentId} />
                <div className="b-title">
                  <div className="b-title__t">{entry.title}</div>
                  <div className="b-title__s">
                    <i>{entry.artist}</i>
                    {entry.album ? ` · ${entry.album}` : ""}
                    {entry.year ? ` · ${entry.year}` : ""}
                    {entry.lyrics ? " · 有歌词" : " · 无歌词"}
                  </div>
                </div>
                <BStamps entry={entry} best={bestEdition(entry)} />
                <span className={`b-qual ${/FLAC/i.test(bestEdition(entry).quality) ? "is-best" : ""}`}>
                  {bestEdition(entry).quality}
                </span>
                <span className="b-dur">{fmtTime(entry.duration)}</span>
                <button className="b-row__more" title="更多操作">
                  <Icon name="more" size={13} />
                </button>
              </div>
            ))
          : flat.map((row) => (
              <div
                key={row.edition.vid}
                className={`b-row b-row--flat ${row.entry.id === currentId && !row.refTo ? "is-current" : ""}`}
                role="button"
                tabIndex={0}
              >
                <span className={`b-row__no ${row.refTo ? "is-ref" : ""}`}>
                  {String(row.no).padStart(2, "0")}
                </span>
                <BCover entry={row.entry} size="sm" playing={row.entry.id === currentId && !row.refTo} />
                <div className="b-title">
                  <div className="b-title__t" style={{ fontSize: 14.5 }}>
                    {row.edition.variant}
                  </div>
                  <div className="b-title__s">
                    {row.refTo ? (
                      <>
                        <span className="b-title__ref">参见 {String(row.refTo).padStart(2, "0")}</span>
                        {" · "}
                      </>
                    ) : null}
                    {row.entry.artist} · {row.edition.uploader}
                  </div>
                </div>
                <BStamps entry={row.entry} best={bestEdition(row.entry)} flat />
                <span className={`b-qual ${(/FLAC/i.test(row.edition.quality)) ? "is-best" : ""}`}>
                  {row.edition.quality}
                </span>
                <span className="b-dur">{fmtTime(row.edition.duration)}</span>
                <button className="b-row__more" title="更多操作">
                  <Icon name="more" size={13} />
                </button>
              </div>
            ))}
      </div>
    </main>
  );
}

function BPage({ currentId }) {
  const entry = ENTRIES.find((e) => e.id === currentId) || ENTRIES[0];
  const found = LYRICS_QINGTIAN.findIndex((l) => l.time === NOW.elapsed);
  const idx = found === -1 ? 14 : found;
  const WINDOW = 16;
  const start = Math.max(0, Math.min(LYRICS_QINGTIAN.length - WINDOW, idx - 5));
  const visible = LYRICS_QINGTIAN.slice(start, start + WINDOW);

  return (
    <aside className="b-page">
      <div className="b-page__head">
        <div className="b-page__hrow">
          <span className="b-page__title">歌词</span>
          <span className="b-num" style={{ fontSize: 10.5, color: "var(--b-ink4)" }}>
            偏移 −0.2s
          </span>
        </div>
        <div className="b-page__prov">
          <span className="b-stamp b-stamp--filled">{entry.lyrics ? entry.lyrics.label : "暂无歌词"}</span>
          {entry.lyrics ? <span className="b-stamp b-stamp--muted">{entry.lyrics.confidence}</span> : null}
        </div>
        <div className="b-page__tools">
          <button className="b-mini is-on">跟随</button>
          <button className="b-mini">显示翻译</button>
        </div>
      </div>

      <div className="b-page__body">
        {visible.map((line, j) => {
          const i = start + j;
          return (
            <div
              key={i}
              className={`b-line ${i === idx ? "is-now" : i < idx ? "is-past" : i - idx === 1 ? "is-next" : ""}`}
            >
              {line.text}
            </div>
          );
        })}
      </div>

      <div className="b-page__foot">
        <div className="b-fnote">
          <b>注一</b>
          <span>歌词回退链：人工 CC 字幕 → 网易云 → 机器字幕。本篇取第一项。</span>
        </div>
        <div className="b-fnote">
          <b>注二</b>
          <span>逐行对齐已应用；点任意行可跳转。</span>
        </div>
      </div>
    </aside>
  );
}

function BFoot({ currentId }) {
  const entry = ENTRIES.find((e) => e.id === currentId) || ENTRIES[0];
  const best = bestEdition(entry);
  const pct = NOW.elapsed / entry.duration;
  const bufPct = Math.max(pct, NOW.buffered / entry.duration);

  return (
    <footer className="b-foot">
      <div className="b-now">
        <span className="b-now__cover" style={{ background: bCoverBg(entry) }}>
          {entry.initial}
        </span>
        <div className="b-now__t">
          <div className="b-now__row">
            <span className="b-now__title">{entry.title}</span>
            <span className="b-stamp b-stamp--filled">{SOURCES[best.source].stare}</span>
            <span className="b-stamp b-stamp--muted">{best.quality}</span>
          </div>
          <div className="b-now__s">
            {entry.artist}
            {entry.album ? ` · ${entry.album}` : ""}
            {entry.year ? ` · ${entry.year}` : ""}
          </div>
        </div>
      </div>

      <div className="b-transport">
        <div className="b-buttons">
          <button className="b-tbtn" title="上一首">
            <Icon name="prev" size={15} />
          </button>
          <button className="b-tbtn is-play" title="播放 / 暂停">
            <Icon name="pause" size={15} />
          </button>
          <button className="b-tbtn" title="下一首">
            <Icon name="next" size={15} />
          </button>
        </div>
        <div className="b-seek">
          <span className="b-seek__t">{fmtTime(NOW.elapsed)}</span>
          <div className="b-seek__bar">
            <div className="b-seek__buf" style={{ width: `${bufPct * 100}%` }} />
            <div className="b-seek__fill" style={{ width: `${pct * 100}%` }} />
            <div className="b-seek__knob" style={{ left: `${pct * 100}%` }} />
          </div>
          <span className="b-seek__t">{fmtTime(entry.duration)}</span>
        </div>
      </div>

      <div className="b-foot__right">
        <div className="b-cachebox">
          <div className="b-cachebox__row">
            <span className="b-lbl">正在缓存</span>
            <span className="b-cachebox__v">
              {NOW.buffered} / {entry.duration} s
            </span>
          </div>
          <div className="b-cachebox__bar">
            <div className="b-cachebox__fill" style={{ width: `${bufPct * 100}%` }} />
          </div>
        </div>

        <div className="b-vol">
          <Icon name="volume" size={14} />
          <div className="b-vol__bar">
            <div className="b-vol__fill" style={{ width: "80%" }} />
          </div>
        </div>

        <button className="b-tbtn" title="播放队列">
          <Icon name="queue" size={15} />
        </button>
      </div>
    </footer>
  );
}

function DirectionB() {
  const [merged, setMerged] = React.useState(initialMerged());
  return (
    <div className="dirb" data-screen-label="B · The Index">
      <BMasthead />
      <BToc />
      <BIndex merged={merged} setMerged={setMerged} currentId={NOW.entryId} />
      <BPage currentId={NOW.entryId} />
      <BFoot currentId={NOW.entryId} />
    </div>
  );
}

Object.assign(window, { DirectionB });

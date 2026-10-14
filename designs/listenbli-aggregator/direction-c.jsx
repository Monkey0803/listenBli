// direction-c.jsx — Direction C · TIDELINE
//
// Several tributaries feed one stream. The rail shows what each is contributing;
// the page is the merged stream itself. Source identity is a filament (a length of
// light in that source's hue), never a badge. Cache state is depth, not a label:
// a downloaded row sits heavier in the water.

const C_TABS = [
  { id: "all", label: "全部", n: ENTRIES.length, on: true },
  { id: "bilibili", label: "B 站", n: 6 },
  { id: "youtube", label: "YouTube", n: 5 },
];

const cFil = (sourceId) => `hsl(${SOURCES[sourceId].hue} 72% 62%)`;
const cShort = (q) => (/FLAC/i.test(q) ? "FLAC" : ((q.match(/(\d+)\s*K/i) || [, q])[1] + "K"));

function CBloom({ entry, size = "md", playing, now }) {
  return (
    <span
      className={`c-bloom ${size === "sm" ? "c-bloom--sm" : ""} ${now ? "c-now__bloom" : ""}`}
      style={{ "--h": trackHue(entry) }}
      aria-hidden="true"
    >
      <span className="c-bloom__glow" />
      {playing ? <Icon name="wave" size={size === "sm" ? 12 : 15} className="c-bloom__eq" /> : entry.initial}
    </span>
  );
}

function CTop() {
  return (
    <header className="c-top">
      <div className="c-brand">
        <span className="c-brand__word">
          listen<i>Bli</i>
        </span>
        <span className="c-brand__sub">TIDELINE</span>
      </div>

      <nav className="c-tabs">
        {C_TABS.map((t) => (
          <button key={t.id} className={`c-tab ${t.on ? "is-on" : ""}`}>
            <span>{t.label}</span>
            <span className="c-tab__n">{t.n}</span>
          </button>
        ))}
      </nav>

      <div className="c-search">
        <Icon name="search" size={13} />
        <input value={QUERY.keyword} readOnly aria-label="搜索" />
        <span className="c-search__key">⌘K</span>
      </div>

      <div className="c-top__right">
        <button className="c-ghost">
          <Icon name="plug" size={12} />
          来源与会话
        </button>
        <button className="c-ghost">
          <Icon name="disk" size={12} />
          <span className="c-mono" style={{ fontSize: 10.5 }}>
            {fmtMb(CACHE.totalMb)}
          </span>
        </button>
        <button className="c-round" title="设置">
          <Icon name="settings" size={13} />
        </button>
        <button className="c-round" title="账号">
          <span style={{ fontFamily: "var(--c-ui)", fontSize: 11 }}>夜</span>
        </button>
      </div>
    </header>
  );
}

function CVein() {
  const level = { bilibili: [0.3, 0.5, 0.4, 0.7, 0.55, 0.85], youtube: [0.2, 0.32, 0.26, 0.4, 0.3, 0.44] };
  return (
    <nav className="c-vein">
      {SOURCE_ORDER.map((id) => {
        const s = SOURCES[id];
        const bars = level[id];
        const on = id === "bilibili";
        return (
          <div className="c-vein__item" key={id} style={{ "--f": cFil(id) }}>
            <span className="c-vein__fil" />
            <span className={`c-vein__mark ${on ? "" : "is-idle"}`} style={{ color: on ? cFil(id) : undefined }}>
              {s.filament}
            </span>
            <span className="c-vein__name">{s.name}</span>
            <span className="c-vein__in" aria-hidden="true">
              {bars.map((v, i) => (
                <i key={i} className={on ? "is-lit" : ""} style={{ height: `${Math.round(v * 30)}px` }} />
              ))}
            </span>
            <span className="c-vein__sess" title={`${s.session.label} · ${s.session.detail}`}>
              <i className={`c-dot ${s.session.kind === "account" ? "is-ok" : "is-anon"}`} />
              <span>{s.session.kind === "account" ? "已登录" : "匿名"}</span>
            </span>
          </div>
        );
      })}

      <div className="c-vein__merge">
        <span className="c-lbl">汇入</span>
        <span className="c-vein__merge-v">{editionCount()}</span>
        <span className="c-vein__merge-u">已合并 {mergedGroupCount()} 组</span>
      </div>

      <button className="c-vein__add" title={CONNECTOR.contract.join(" · ")}>
        <Icon name="plus" size={14} />
        <span>{CONNECTOR.label}</span>
      </button>
    </nav>
  );
}

function CStream({ merged, setMerged, currentId }) {
  let n = 0;
  const flat = ENTRIES.flatMap((entry) => {
    const first = n + 1;
    return entry.editions.map((edition, i) => ({ entry, edition, no: ++n, refTo: i > 0 ? first : null }));
  });

  return (
    <main className="c-stream">
      <div className="c-stream__head">
        <div>
          <div className="c-stream__q">
            <h1>{QUERY.keyword}</h1>
          </div>
          <div className="c-stream__meta">
            <span className="c-stream__m">
              <b>{ENTRIES.length}</b> 条著录
            </span>
            <span className="c-stream__sep" />
            <span className="c-stream__m">
              <b>{QUERY.sources}</b> 条流
            </span>
            <span className="c-stream__sep" />
            <span className="c-stream__m">
              已合并 <b>{mergedGroupCount()}</b> 组
            </span>
            <span className="c-stream__sep" />
            <span className="c-stream__m">{QUERY.took.toFixed(2)}s</span>
          </div>
        </div>

        <div style={{ flex: "1 1 auto" }} />

        <button className={`c-toggle ${merged ? "is-on" : ""}`} onClick={() => setMerged(!merged)}>
          <span className="c-toggle__sw">
            <i />
          </span>
          <span className="c-toggle__t">合并重复</span>
        </button>

        <button className="c-play">
          <Icon name="play" size={11} />
          全部播放
        </button>
      </div>

      <div className="c-stream__note">
        <Icon name="merge" size={14} style={{ color: "var(--c-cyan)", flex: "none" }} />
        <span>
          {merged ? (
            <>
              同一条录音只出现<b>一次</b>；它来自哪几条支流写在右栏，行尾是合并记号。
              播放取音质最高、已缓存的那个。
            </>
          ) : (
            <>
              已按来源拆开为 <b>{editionCount()} 行</b>；属于同一录音的行由左缘的
              <em>收束细丝</em>标出，第二条起以 <em>汇入</em> 回指首行。
            </>
          )}
        </span>
      </div>

      <div className="c-rows">
        {merged
          ? ENTRIES.map((entry) => {
              const best = bestEdition(entry);
              const cached = best.cache.done;
              const isNow = entry.id === currentId;
              return (
                <div
                  key={entry.id}
                  className={`c-row c-row--merged ${cached ? "is-cached" : ""} ${isNow ? "is-current" : ""}`}
                  role="button"
                  tabIndex={0}
                >
                  <CBloom entry={entry} playing={isNow} />

                  <div className="c-title">
                    <div className="c-title__t">
                      {entry.title}
                      {entry.lyrics ? <span className="c-title__lyr">词</span> : null}
                    </div>
                    <div className="c-title__s">
                      <i>{entry.artist}</i>
                      {entry.album ? ` · ${entry.album}` : ""}
                      {entry.year ? ` · ${entry.year}` : ""}
                    </div>
                  </div>

                  <div className="c-src">
                    {entry.editions.length > 1 ? (
                      <span className="c-converge" style={{ "--f": cFil(best.source) }} aria-hidden="true">
                        <i />
                        <i />
                      </span>
                    ) : null}
                    {entry.editions.map((e) => (
                      <span className="c-src__one" key={e.vid} style={{ "--f": cFil(e.source) }}>
                        <i className="c-fil" />
                        <span className="c-src__mark">{SOURCES[e.source].filament}</span>
                        <span className="c-src__q">{cShort(e.quality)}</span>
                      </span>
                    ))}
                  </div>

                  <span className="c-qual">{best.quality}</span>
                  <span className="c-dur">{fmtTime(entry.duration)}</span>

                  <button className="c-row__more" title="更多操作">
                    <Icon name="more" size={14} />
                  </button>

                  <span
                    className={`c-depthbar ${best.cache.done ? "is-full" : best.cache.partial ? "is-partial" : ""}`}
                    title={best.cache.done ? "已完整缓存" : best.cache.partial ? "部分缓存" : "未缓存"}
                  />
                </div>
              );
            })
          : flat.map((row) => {
              const isNow = row.entry.id === currentId && !row.refTo;
              const cached = row.edition.cache.done;
              return (
                <div className="c-group" key={row.edition.vid} style={{ "--gf": cFil(row.edition.source) }}>
                  <div
                    className={`c-row c-row--flat c-row--in-group ${cached ? "is-cached" : ""} ${isNow ? "is-current" : ""}`}
                    role="button"
                    tabIndex={0}
                  >
                    <CBloom entry={row.entry} size="sm" playing={isNow} />

                    <div className="c-title">
                      <div className="c-title__t" style={{ fontSize: 14.5 }}>
                        {row.edition.variant}
                      </div>
                      <div className="c-title__s">
                        {row.refTo ? <span className="c-src__merge">汇入 {String(row.refTo).padStart(2, "0")} · </span> : null}
                        {row.entry.artist} · {row.edition.uploader}
                      </div>
                    </div>

                    <div className="c-src">
                      <span className="c-src__one" style={{ "--f": cFil(row.edition.source) }}>
                        <i className="c-fil" />
                        <span className="c-src__mark">{SOURCES[row.edition.source].filament}</span>
                        <span className="c-src__q">{cShort(row.edition.quality)}</span>
                      </span>
                    </div>

                    <span className="c-qual">{row.edition.quality}</span>
                    <span className="c-dur">{fmtTime(row.edition.duration)}</span>

                    <button className="c-row__more" title="更多操作">
                      <Icon name="more" size={14} />
                    </button>

                    <span
                      className={`c-depthbar ${
                        row.edition.cache.done ? "is-full" : row.edition.cache.partial ? "is-partial" : ""
                      }`}
                      title={
                        row.edition.cache.done ? "已完整缓存" : row.edition.cache.partial ? "部分缓存" : "未缓存"
                      }
                    />
                  </div>
                </div>
              );
            })}
      </div>
    </main>
  );
}

function CVoice({ currentId }) {
  const entry = ENTRIES.find((e) => e.id === currentId) || ENTRIES[0];
  const found = LYRICS_QINGTIAN.findIndex((l) => l.time === NOW.elapsed);
  const idx = found === -1 ? 14 : found;
  const WINDOW = 15;
  const start = Math.max(0, Math.min(LYRICS_QINGTIAN.length - WINDOW, idx - 5));
  const visible = LYRICS_QINGTIAN.slice(start, start + WINDOW);

  return (
    <aside className="c-voice">
      <div className="c-voice__head">
        <div className="c-voice__hrow">
          <span className="c-voice__title">声音</span>
          <span className="c-mono" style={{ fontSize: 10, color: "var(--c-fg4)" }}>
            偏移 −0.2s
          </span>
        </div>
        <div className="c-voice__prov">
          <span className="c-prov is-on">
            <i className="c-dot" style={{ background: cFil("bilibili") }} />
            {entry.lyrics ? entry.lyrics.label : "暂无歌词"}
          </span>
          {entry.lyrics ? <span className="c-prov">{entry.lyrics.confidence}</span> : null}
        </div>
        <div className="c-voice__tools">
          <button className="c-mini is-on">跟随</button>
          <button className="c-mini">显示翻译</button>
        </div>
      </div>

      <div className="c-voice__body">
        {visible.map((line, j) => {
          const i = start + j;
          return (
            <div
              key={i}
              className={`c-line ${i === idx ? "is-now" : i < idx ? "is-past" : i - idx === 1 ? "is-next" : ""}`}
            >
              {line.text}
            </div>
          );
        })}
      </div>

      <div className="c-voice__foot">
        <div className="c-vr">
          <span className="c-lbl">回退链</span>
          <span className="c-vr__v">CC → 网易云 → ASR</span>
        </div>
        <div className="c-vr">
          <span className="c-lbl">对齐</span>
          <span className="c-vr__v is-cyan">逐行 · 已对齐</span>
        </div>
      </div>
    </aside>
  );
}

function CDeck({ currentId }) {
  const entry = ENTRIES.find((e) => e.id === currentId) || ENTRIES[0];
  const best = bestEdition(entry);
  const pct = NOW.elapsed / entry.duration;
  const bufPct = Math.max(pct, NOW.buffered / entry.duration);

  return (
    <div className="c-deck">
      <div className="c-deck__main">
        <div className="c-now">
          <CBloom entry={entry} now playing={false} />
          <div className="c-now__t">
            <div className="c-now__row">
              <span className="c-now__title">{entry.title}</span>
              <span className="c-src__one" style={{ "--f": cFil(best.source) }}>
                <i className="c-fil" />
                <span className="c-src__mark">{SOURCES[best.source].filament}</span>
              </span>
              <span className="c-src__q">{best.quality}</span>
            </div>
            <div className="c-now__s">
              {entry.artist}
              {entry.album ? ` · ${entry.album}` : ""}
              {entry.year ? ` · ${entry.year}` : ""}
            </div>
          </div>
        </div>

        <div className="c-transport">
          <div className="c-buttons">
            <button className="c-tbtn" title="上一首">
              <Icon name="prev" size={14} />
            </button>
            <button className="c-tbtn is-play" title="播放 / 暂停">
              <Icon name="pause" size={15} />
            </button>
            <button className="c-tbtn" title="下一首">
              <Icon name="next" size={14} />
            </button>
          </div>
          <div className="c-seek">
            <span className="c-seek__t">{fmtTime(NOW.elapsed)}</span>
            <div className="c-seek__bar">
              <div className="c-seek__buf" style={{ width: `${bufPct * 100}%` }} />
              <div className="c-seek__fill" style={{ width: `${pct * 100}%` }} />
              <div className="c-seek__knob" style={{ left: `calc(${pct * 100}% - 4px)` }} />
            </div>
            <span className="c-seek__t">{fmtTime(entry.duration)}</span>
          </div>
        </div>

        <div className="c-deck__right">
          <div className="c-depth">
            <div className="c-depth__row">
              <span className="c-lbl">入水</span>
              <span className="c-depth__v">
                {NOW.buffered} / {entry.duration} s
              </span>
            </div>
            <div className="c-depth__bar">
              <div className="c-depth__fill" style={{ width: `${bufPct * 100}%` }} />
            </div>
          </div>

          <div className="c-vol">
            <Icon name="volume" size={14} />
            <div className="c-vol__bar">
              <div className="c-vol__fill" style={{ width: "80%" }} />
            </div>
          </div>
        </div>
      </div>

      <div className="c-film">
        <span className="c-lbl" style={{ marginRight: 4 }}>
          接下来 {QUEUE.length}
        </span>
        {QUEUE.map((q, i) => (
          <button key={i} className={`c-film__item ${q.state === "playing" ? "is-now" : ""}`}>
            <i className="c-fil" style={{ "--f": cFil(q.source), width: 12 }} />
            <span className="c-film__t">{q.title}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

function DirectionC() {
  const [merged, setMerged] = React.useState(initialMerged());
  return (
    <div className="dirc" data-screen-label="C · Tideline">
      <CTop />
      <CVein />
      <CStream merged={merged} setMerged={setMerged} currentId={NOW.entryId} />
      <CVoice currentId={NOW.entryId} />
      <CDeck currentId={NOW.entryId} />
    </div>
  );
}

Object.assign(window, { DirectionC });

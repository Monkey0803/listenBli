// direction-a.jsx — Direction A · SWITCHBOARD
//
// Sources are input channels on a broadcast console. Channel identity is a letter
// and an LED; amber is the only signal colour and it always means "on air".
//
// Owns one piece of state: whether cross-source duplicates are merged into one
// entry or listed per source. Every direction duplicates that toggle deliberately,
// because it is the comparison's control.

const A_TABS = [
  { id: "all", label: "全部", n: ENTRIES.length },
  { id: "bilibili", label: "B 站", n: ENTRIES.filter((e) => e.editions.some((x) => x.source === "bilibili")).length },
  { id: "youtube", label: "YouTube", n: ENTRIES.filter((e) => e.editions.some((x) => x.source === "youtube")).length },
];

/** FLAC 无损 → FLAC ; 192K AAC-LC → 192K */
const aShortQual = (q) => {
  if (/FLAC/i.test(q)) return "FLAC";
  const m = q.match(/(\d+)\s*K/i);
  return m ? `${m[1]}K` : q;
};

const ACacheDot = ({ done, partial }) => (
  <span className={`a-led ${partial ? "is-warn" : done ? "is-ok" : "is-idle"}`} title={partial ? "部分缓存" : done ? "已缓存" : "未缓存"} />
);

function ACover({ entry, size = "md", playing }) {
  const h = trackHue(entry);
  return (
    <span
      className={`a-cover ${size === "sm" ? "a-cover--sm" : ""}`}
      style={{ background: `linear-gradient(142deg, hsl(${h} 15% 25%), hsl(${h} 20% 12%))` }}
    >
      {entry.initial}
      {playing ? (
        <span className="a-cover__eq">
          <Icon name="wave" size={size === "sm" ? 11 : 14} />
        </span>
      ) : null}
    </span>
  );
}

// The console's page keys. On a hardware desk these live on the master section;
// here they sit in the left rail, under the input channels.
const A_PAGES = [
  { id: "mix", label: "监听", tag: "MIX" },
  { id: "queue", label: "队列", tag: "QUEUE" },
  { id: "disk", label: "存储", tag: "DISK" },
  { id: "input", label: "输入", tag: "INPUT" },
];

function ATopBar({ page, onPage, scope, onScope }) {
  const current = A_PAGES.find((p) => p.id === page);
  return (
    <header className="a-top">
      <div className="a-brand">
        <span className="a-brand__bars">
          <Icon name="wave" size={20} />
        </span>
        <span className="a-brand__word">
          listen<i>Bli</i>
        </span>
        <span className="a-brand__tag">{current ? `${current.tag} · ${current.label}` : "Mixer"}</span>
      </div>

      <div className="a-switch">
        {A_TABS.map((t) => {
          const active = (scope ?? "all") === t.id;
          return (
            <button
              key={t.id}
              className={`a-switch__cell ${active ? "is-on" : ""}`}
              onClick={() => onScope && onScope(t.id)}
            >
              <span className="a-switch__led">
                <span className={`a-led ${active ? "is-on" : "is-idle"}`} />
              </span>
              <span>{t.label}</span>
              <span className="a-switch__n">{t.n}</span>
            </button>
          );
        })}
      </div>

      <div className="a-search">
        <Icon name="search" size={13} />
        <input value={QUERY.keyword} readOnly aria-label="搜索" />
        <span className="a-search__key">⌘K</span>
      </div>

      <button className="a-chip" title="缓存与离线" onClick={() => onPage && onPage("disk")}>
        <Icon name="disk" size={13} />
        <span className="a-eng">Cache</span>
        <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg3)" }}>
          {fmtMb(CACHE.totalMb)}
        </span>
      </button>

      <button className="a-chip" title="来源管理与会话" onClick={() => onPage && onPage("input")}>
        <Icon name="plug" size={13} />
        <span className="a-eng">Sources</span>
        <span className="a-mono" style={{ fontSize: 10, color: "var(--a-green)" }}>
          2/2
        </span>
      </button>

      <button className="a-iconbtn" title="设置">
        <Icon name="settings" size={14} />
      </button>

      <button className="a-account" title="账号">
        <span className="a-account__av">夜</span>
        <span className="a-account__t">
          <span className="a-account__n">夜航西飞</span>
          <span className="a-account__r">大会员</span>
        </span>
      </button>
    </header>
  );
}

function AChannel({ source, live, level, levelHot, cached, total, ratio, bufferLabel, partial }) {
  const s = SOURCES[source];
  const bars = [0.36, 0.62, 0.44, 0.78, 0.52, 0.9, 0.4, 0.68, 0.48, 0.84, 0.58, 0.72];
  return (
    <div className={`a-chan ${live ? "is-live" : ""}`}>
      <div className="a-chan__top">
        <span className="a-chan__letter">{s.channel}</span>
        <span className="a-chan__name">{s.name}</span>
        <span className={`a-led ${live ? "is-on" : "is-ok"}`} />
      </div>

      <div className="a-chan__sess">
        <span className="a-led is-ok" />
        <span className="a-chan__sess-t">{s.session.label}</span>
        <span className="a-chan__sess-d">{s.session.detail}</span>
      </div>

      <div className="a-meter" aria-hidden="true">
        {bars.map((v, i) => {
          const h = live ? Math.min(1, v * level) : 0.12;
          const hot = h > 0.7;
          return (
            <i
              key={i}
              className={`${live ? "is-lit" : ""} ${live && hot ? "is-hot" : ""}`}
              style={{ "--h": `${Math.round(h * 100)}%` }}
            />
          );
        })}
      </div>

      <div className="a-buf">
        <div className="a-buf__row">
          <span className="a-eng">Local</span>
          <span className="a-buf__t">
            缓存 {cached} / {total} 首
          </span>
        </div>
        <div className="a-buf__bar">
          <div className={`a-buf__fill ${ratio <= 0 ? "is-none" : partial ? "is-partial" : ""}`} style={{ width: `${Math.round(ratio * 100)}%` }} />
        </div>
      </div>

      <div className="a-buf" style={{ marginTop: 7 }}>
        <div className="a-buf__row">
          <span className="a-eng">Buffer</span>
          <span className="a-buf__t">{bufferLabel}</span>
        </div>
      </div>
    </div>
  );
}

function AInputRail({ page, onPage, onConnect }) {
  return (
    <aside className="a-rail">
      <div className="a-rail__head">
        <span className="a-eng">Inputs</span>
        <span className="a-eng">2 online</span>
      </div>

      <AChannel
        source="bilibili"
        live
        level={0.92}
        cached={3}
        total={6}
        ratio={0.62}
        bufferLabel="188 / 269 s"
      />
      <AChannel source="youtube" live={false} level={0.42} cached={1} total={5} ratio={0.28} bufferLabel="空闲" />

      {onPage ? (
        <div className="a-pages">
          <span className="a-eng">Desk</span>
          <div className="a-pages__keys">
            {A_PAGES.map((p) => (
              <button
                key={p.id}
                className={`a-pagekey ${page === p.id ? "is-on" : ""}`}
                onClick={() => onPage(p.id)}
              >
                <span className="a-pagekey__led">
                  <span className={`a-led ${page === p.id ? "is-on" : "is-idle"}`} />
                </span>
                <span className="a-pagekey__t">{p.label}</span>
                <span className="a-pagekey__k">{p.tag}</span>
              </button>
            ))}
          </div>
        </div>
      ) : null}

      <div className="a-master">
        <span className="a-eng">Monitor</span>
        <div className="a-master__out" aria-hidden="true">
          {[0.3, 0.52, 0.4, 0.74, 0.56, 0.88, 0.46, 0.66, 0.38, 0.58].map((v, i) => (
            <i key={i} style={{ "--h": `${Math.round(v * 100)}%` }} />
          ))}
        </div>

        <div className="a-readout">
          <span className="a-eng">重复已合并</span>
          <span className="a-readout__v is-amber">{mergedGroupCount()} 组</span>
        </div>
        <div className="a-readout">
          <span className="a-eng">著录 / 来源条目</span>
          <span className="a-readout__v">
            {ENTRIES.length} / {editionCount()}
          </span>
        </div>
        <div className="a-readout">
          <span className="a-eng">磁盘</span>
          <span className="a-readout__v is-green">
            {fmtMb(CACHE.totalMb)}/{fmtMb(CACHE.limitMb)}
          </span>
        </div>
      </div>

      <button className="a-connect" onClick={() => onConnect && onConnect()}>
        <span className="a-connect__top">
          <Icon name="plus" size={14} />
          <span className="a-connect__t">{CONNECTOR.label}</span>
        </span>
        <span className="a-connect__c">{CONNECTOR.contract.join(" · ")}</span>
      </button>
    </aside>
  );
}

function ASourceChip({ edition, on }) {
  const s = SOURCES[edition.source];
  return (
    <span className={`a-src__chip ${on ? "is-on" : ""}`}>
      <b>{s.channel}</b>
      <em>{aShortQual(edition.quality)}</em>
      <ACacheDot done={edition.cache.done} partial={edition.cache.partial} />
    </span>
  );
}

function AMergedRow({ entry, current }) {
  const best = bestEdition(entry);
  const bestSource = SOURCES[best.source];
  const hasLyrics = !!entry.lyrics;
  return (
    <div
      className={`a-row a-row--merged ${current ? "is-current" : ""}`}
      role="button"
      tabIndex={0}
      aria-label={`播放 ${entry.title}`}
    >
      <div className="a-ch">
        <div className="a-ch__stack">
          <span>
            <b>{bestSource.channel}</b>
          </span>
          {entry.editions.length > 1 ? (
            <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
              +{entry.editions.length - 1}
            </span>
          ) : null}
        </div>
      </div>

      <ACover entry={entry} playing={current} />

      <div className="a-title">
        <div className="a-title__t">
          {entry.title}
          {hasLyrics ? <span className="a-title__flag">词</span> : null}
        </div>
        <div className="a-title__s">
          <i>{entry.artist}</i>
          {entry.album ? ` · ${entry.album}` : ""}
          {entry.year ? ` · ${entry.year}` : ""}
        </div>
      </div>

      <div className="a-src">
        {entry.editions.map((e) => (
          <ASourceChip key={e.vid} edition={e} on={e === best} />
        ))}
      </div>

      <div className="a-qual">
        <span className={`a-qual__t ${/FLAC/i.test(best.quality) ? "is-flac" : ""}`}>{best.quality}</span>
      </div>

      <div className="a-dur">{fmtTime(entry.duration)}</div>

      <button className="a-row__more" title="更多操作">
        <Icon name="more" size={14} />
      </button>
    </div>
  );
}

function AFlatRow({ entry, edition, index, current, showGroupNote }) {
  const s = SOURCES[edition.source];
  return (
    <>
      <div className={`a-row a-row--flat a-row--in-group ${current ? "is-current" : ""}`} role="button" tabIndex={0}>
        <div className="a-ch">
          <span>
            <b>{s.channel}</b>
          </span>
          <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg4)" }}>
            {String(index + 1).padStart(2, "0")}
          </span>
        </div>

        <ACover entry={entry} size="sm" playing={current} />

        <div className="a-title">
          <div className="a-title__t">{edition.variant}</div>
          <div className="a-title__s">
            {showGroupNote ? (
              <>
                <span style={{ color: "var(--a-amber)" }}>
                  <Icon name="merge" size={10} weight={1.8} />
                </span>{" "}
                <i>与上一行同一首录音</i> ·{" "}
              </>
            ) : null}
            {entry.artist} · {edition.uploader}
          </div>
        </div>

        <div className="a-src">
          <ASourceChip edition={edition} on />
        </div>

        <div className="a-qual">
          <span className={`a-qual__t ${/FLAC/i.test(edition.quality) ? "is-flac" : ""}`}>{edition.quality}</span>
        </div>

        <div className="a-dur">{fmtTime(edition.duration)}</div>

        <button className="a-row__more" title="更多操作">
          <Icon name="more" size={14} />
        </button>
      </div>
    </>
  );
}

function AResultList({ merged, setMerged, currentId, scope = "all", onOpenQueue }) {
  // A per-source tab is not "the same list filtered": it is that source's own
  // holdings, so the merge control has nothing to merge and is taken away rather
  // than shown disabled. This is the difference between a filter and a tab.
  const scoped = scope !== "all";
  const entries = scoped ? ENTRIES.filter((e) => e.editions.some((x) => x.source === scope)) : ENTRIES;
  const shownEditions = scoped ? entries.reduce((n, e) => n + e.editions.filter((x) => x.source === scope).length, 0) : editionCount();
  const flat = scoped || !merged;
  return (
    <main className="a-list">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>{scoped ? QUERY.keyword : QUERY.keyword}</h1>
            {scoped ? <span className="a-tag is-dim">{SOURCES[scope].short} 条目</span> : null}
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{scoped ? shownEditions : entries.length}</b> {scoped ? "个来源条目" : "条著录"}
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              {scoped ? (
                <>
                  最高音质 <b>{bestEdition(entries[0]).quality}</b>
                </>
              ) : (
                <>
                  <b>{mergedGroupCount()}</b> 组合并
                </>
              )}
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              {scoped ? "仅此来源" : `${QUERY.sources} 个来源`} · {QUERY.took.toFixed(2)}s
            </span>
          </div>
        </div>

        <div style={{ flex: "1 1 auto" }} />

        {scoped ? (
          <span className="a-tag is-dim" title="单来源视图里没有可合并的对象">
            <Icon name="lock" size={11} />
            单来源视图 · 无合并
          </span>
        ) : (
          <button className={`a-toggle ${merged ? "is-on" : ""}`} onClick={() => setMerged(!merged)}>
            <span className="a-toggle__sw">
              <i />
            </span>
            <span className="a-toggle__t">合并重复</span>
          </button>
        )}

        <button className="a-btn is-primary">
          <Icon name="play" size={11} />
          全部播放
        </button>
      </div>

      <div className="a-cols">
        <span className="a-eng">Ch</span>
        <span />
        <span className="a-eng">标题</span>
        <span className="a-eng">来源</span>
        <span className="a-eng">音质</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          时长
        </span>
        <span />
      </div>

      <div className="a-rows">
        <div className="a-note">
          <Icon
            name={scoped ? "filter" : "merge"}
            size={14}
            style={{ color: scoped ? "var(--a-fg4)" : merged ? "var(--a-amber)" : "var(--a-fg4)", flex: "none" }}
          />
          <span className="a-note__t">
            {scoped ? (
              <>
                这是 <b>{SOURCES[scope].name}</b> 自己的条目列表（<b>{shownEditions}</b> 条）—— 只列这个来源的持有，
                不做跨源合并，「合并重复」在这里没有意义，所以直接撤掉而不是置灰。
              </>
            ) : merged ? (
              <>
                已合并 <b>{mergedGroupCount()} 组</b>跨源重复 —— 同一首录音在 B 站与 YouTube 各有一个条目时，列表中只占一行，
                播放取<b>音质最高、已缓存优先</b>的那个。
              </>
            ) : (
              <>
                已展开 <b>{editionCount()} 个</b>来源条目 —— 左侧竖线标出属于同一首录音的行。
              </>
            )}
          </span>
        </div>

        {flat
          ? entries.map((entry) => (
              <div className="a-group" key={entry.id}>
                {entry.editions
                  .filter((x) => !scoped || x.source === scope)
                  .map((edition, i) => (
                    <AFlatRow
                      key={edition.vid}
                      entry={entry}
                      edition={edition}
                      index={i}
                      current={entry.id === currentId && i === 0}
                      showGroupNote={!scoped && i > 0}
                    />
                  ))}
              </div>
            ))
          : entries.map((entry) => (
              <AMergedRow key={entry.id} entry={entry} current={entry.id === currentId} />
            ))}
      </div>

      <div className="a-queue">
        <div className="a-queue__head">
          <span className="a-eng">Up next · {QUEUE.length} in queue</span>
          <button className="a-eng" style={{ color: "var(--a-fg2)" }} onClick={() => onOpenQueue && onOpenQueue()}>
            打开队列 ▸
          </button>
        </div>
        <div className="a-queue__row">
          {QUEUE.map((q, i) => (
            <button key={i} className={`a-queue__item ${q.state === "playing" ? "is-now" : ""}`}>
              <span className={`a-led ${q.state === "playing" ? "is-on" : q.state === "next" ? "is-ok" : "is-idle"}`} />
              <span className="a-queue__l">{SOURCES[q.source].channel}</span>
              <span className="a-queue__t">{q.title}</span>
            </button>
          ))}
        </div>
      </div>
    </main>
  );
}

function ALyricFeed({ currentId }) {
  const entry = ENTRIES.find((e) => e.id === currentId) || ENTRIES[0];
  const found = LYRICS_QINGTIAN.findIndex((l) => l.time === NOW.elapsed);
  const idx = found === -1 ? 14 : found;
  // A lyric pane follows the singing line; it does not start at the top of the
  // song. Show a fixed window with the current line in the upper third.
  const WINDOW = 15;
  const start = Math.max(0, Math.min(LYRICS_QINGTIAN.length - WINDOW, idx - 6));
  const visible = LYRICS_QINGTIAN.slice(start, start + WINDOW);
  return (
    <aside className="a-lyric">
      <div className="a-lyric__head">
        <div className="a-lyric__hrow">
          <span className="a-eng">Lyric feed</span>
          <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg4)" }}>
            偏移 −0.2s
          </span>
        </div>
        <div className="a-lyric__src">
          <span className="a-tag is-amber">
            <span className="a-led is-on" />
            {entry.lyrics ? entry.lyrics.label : "暂无歌词"}
          </span>
          {entry.lyrics ? <span className="a-tag is-dim">{entry.lyrics.confidence}</span> : null}
        </div>
        <div style={{ display: "flex", gap: 6, marginTop: 9 }}>
          <button className="a-pill is-on">跟随</button>
          <button className="a-pill">显示翻译</button>
        </div>
      </div>

      <div className="a-lyric__body">
        {visible.map((line, j) => {
          const i = start + j;
          return (
            <div
              key={i}
              className={`a-line ${i === idx ? "is-now" : i < idx ? "is-past" : i - idx === 1 ? "is-next" : ""}`}
            >
              {line.text}
            </div>
          );
        })}
      </div>

      <div className="a-lyric__foot">
        <div className="a-readout">
          <span className="a-eng">回退链</span>
          <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg3)" }}>
            CC → 网易云 → ASR
          </span>
        </div>
        <div className="a-readout">
          <span className="a-eng">对齐</span>
          <span className="a-readout__v is-green">逐行 · 已对齐</span>
        </div>
      </div>
    </aside>
  );
}

function ADeck({ currentId }) {
  const entry = ENTRIES.find((e) => e.id === currentId) || ENTRIES[0];
  const best = bestEdition(entry);
  const pct = NOW.elapsed / entry.duration;
  const bufPct = Math.max(pct, NOW.buffered / entry.duration);
  return (
    <footer className="a-deck">
      <div className="a-now">
        <span
          className="a-now__cover"
          style={{ background: `linear-gradient(142deg, hsl(${trackHue(entry)} 15% 26%), hsl(${trackHue(entry)} 20% 12%))` }}
        >
          {entry.initial}
        </span>
        <div className="a-now__t">
          <div className="a-now__row">
            <span className="a-now__title">{entry.title}</span>
            <span className="a-tag is-amber">
              <span className="a-led is-on" />
              {SOURCES[best.source].channel} · {SOURCES[best.source].short}
            </span>
            <span className="a-tag">{best.quality}</span>
          </div>
          <div className="a-now__s">
            {entry.artist}
            {entry.album ? ` · ${entry.album}` : ""}
            {entry.year ? ` · ${entry.year}` : ""}
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
          <span className="a-seek__t">{fmtTime(entry.duration)}</span>
        </div>
      </div>

      <div className="a-deck__right">
        <div className="a-deck__box">
          <div className="a-buf__row">
            <span className="a-eng">Streaming</span>
            <span className="a-buf__t">
              {NOW.buffered} / {entry.duration} s
            </span>
          </div>
          <div className="a-buf__bar">
            <div className={`a-buf__fill ${best.cache.done ? "" : "is-partial"}`} style={{ width: `${bufPct * 100}%` }} />
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

function DirectionA() {
  const [merged, setMerged] = React.useState(initialMerged());
  const currentId = NOW.entryId;
  return (
    <div className="dira" data-screen-label="A · Switchboard">
      <ATopBar />
      <AInputRail />
      <AResultList merged={merged} setMerged={setMerged} currentId={currentId} />
      <ALyricFeed currentId={currentId} />
      <ADeck currentId={currentId} />
    </div>
  );
}

// The sub-components are exported so the deeper prototype (mixer-app.jsx) can
// compose the same console around extra pages instead of forking the whole
// direction and letting the two drift apart.
Object.assign(window, {
  DirectionA,
  A_TABS,
  A_PAGES,
  ATopBar,
  AInputRail,
  AChannel,
  AResultList,
  ALyricFeed,
  ADeck,
  ACover,
  ASourceChip,
  ACacheDot,
  aShortQual,
});

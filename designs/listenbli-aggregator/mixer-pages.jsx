// mixer-pages.jsx — the four pages the console gains once a direction is chosen:
// 队列 / 存储 / 输入 / 接入来源. Written in direction A's language (see mixer.css).
//
// These are pages, not dialogs: a desk changes what the workspace shows with page
// keys, and the transport deck and input meters never move. The connect flow is
// the one exception — inserting a module is a thing you do *to* the desk, so it
// takes the whole surface.

// ---------------------------------------------------------------------------
// 队列
// ---------------------------------------------------------------------------

// The queue page seeds its own longer list. QUEUE stays 6 items because the
// aggregate artboards render it as a compact ribbon; a real queue is longer.
const MQ_SEED = [
  ...QUEUE,
  { title: "Lemon", artist: "米津玄師", source: "bilibili", duration: 256, state: "queued", origin: "收藏夹" },
  { title: "青花瓷", artist: "周杰伦", source: "bilibili", duration: 239, state: "queued", origin: "搜索" },
  { title: "晴天（钢琴改编版）", artist: "Animenz Piano", source: "bilibili", duration: 312, state: "queued", origin: "搜索" },
  { title: "七里香", artist: "周杰伦", source: "bilibili", duration: 299, state: "queued", origin: "收藏夹" },
];

const MQ_LOOPS = [
  { id: "seq", label: "顺序" },
  { id: "one", label: "单曲" },
  { id: "shuffle", label: "随机" },
];

function mqAlternations(items) {
  let n = 0;
  for (let i = 1; i < items.length; i++) if (items[i].source !== items[i - 1].source) n++;
  return n;
}

function MQueuePage({ onOpenMix }) {
  const [items, setItems] = React.useState(MQ_SEED);
  const [loop, setLoop] = React.useState("seq");
  const total = items.reduce((n, q) => n + q.duration, 0);

  const removeAt = (index) => setItems(items.filter((_, j) => j !== index));
  const promote = (index) => {
    if (index <= 1) return;
    const next = items.slice();
    const [it] = next.splice(index, 1);
    next.splice(1, 0, it);
    setItems(next);
  };

  return (
    <main className="a-list mq">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>播放队列</h1>
            <span className="a-tag">跨来源混排</span>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{items.length}</b> 首
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              总时长 <b>{fmtLong(total)}</b>
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              {QUERY.sources} 个来源 · {mqAlternations(items)} 处来源交替
            </span>
          </div>
        </div>

        <div style={{ flex: "1 1 auto" }} />

        <div className="a-seg">
          {MQ_LOOPS.map((l) => (
            <button
              key={l.id}
              className={`a-seg__cell ${loop === l.id ? "is-on" : ""}`}
              onClick={() => setLoop(l.id)}
            >
              {l.label}
            </button>
          ))}
        </div>

        <button className="a-btn" onClick={() => setItems(items.filter((q) => q.state === "playing"))}>
          <Icon name="close" size={11} />
          清空
        </button>
      </div>

      <div className="a-cols mq__cols">
        <span className="a-eng">#</span>
        <span />
        <span className="a-eng">标题</span>
        <span className="a-eng">来源 / 音质</span>
        <span className="a-eng">入队</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          时长
        </span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          操作
        </span>
      </div>

      <div className="a-rows">
        {items.map((q, i) => {
          const s = SOURCES[q.source];
          const now = q.state === "playing";
          return (
            <div key={`${q.title}-${i}`} className={`mq__row ${now ? "is-now" : ""}`}>
              <span className="mq__no">
                <span className={`a-led ${now ? "is-on" : q.state === "next" ? "is-ok" : "is-idle"}`} />
                {String(i + 1).padStart(2, "0")}
              </span>

              <span
                className="a-cover a-cover--sm"
                style={{ background: `linear-gradient(142deg, hsl(${32 + i * 26} 15% 25%), hsl(${32 + i * 26} 20% 12%))` }}
              >
                {q.title.slice(0, 1)}
              </span>

              <div className="a-title">
                <div className="a-title__t">
                  {q.title}
                  {now ? <span className="a-title__flag">在播</span> : null}
                </div>
                <div className="a-title__s">{q.artist}</div>
              </div>

              <div className="a-src">
                <span className={`a-src__chip ${now ? "is-on" : ""}`}>
                  <b>{s.channel}</b>
                  <em>{s.short}</em>
                </span>
                <span className="a-src__chip">
                  <em>{i % 2 === 0 ? "无损 FLAC" : "AAC 130K"}</em>
                </span>
              </div>

              <span className="mq__origin">{q.origin}</span>
              <span className="a-dur">{fmtTime(q.duration)}</span>

              <span className="mq__acts">
                <button
                  className="a-icobtn"
                  title="下一首播放"
                  disabled={now}
                  style={{ opacity: now ? 0.3 : 1 }}
                  onClick={() => promote(i)}
                >
                  <Icon name="chevronDown" size={13} />
                </button>
                <button
                  className="a-icobtn is-danger"
                  title={now ? "正在播放，不能移除" : "从队列移除"}
                  disabled={now}
                  style={{ opacity: now ? 0.3 : 1 }}
                  onClick={() => removeAt(i)}
                >
                  <Icon name="close" size={12} />
                </button>
              </span>
            </div>
          );
        })}
      </div>

      <div className="mq__foot">
        <Icon name="queue" size={13} style={{ color: "var(--a-fg4)" }} />
        <span className="a-eng">跨来源连续播放</span>
        <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
          走到来源交替处会重新解析直链并握一次会话，队列位置不变
        </span>
        <div style={{ flex: "1 1 auto" }} />
        <button className="a-btn" onClick={() => onOpenMix && onOpenMix()}>
          回到监听 ▸
        </button>
      </div>
    </main>
  );
}

function MQueueSide({ items }) {
  const list = items ?? MQ_SEED;
  const counts = SOURCE_ORDER.map((id) => {
    const rows = list.filter((q) => q.source === id);
    return { id, n: rows.length, secs: rows.reduce((a, q) => a + q.duration, 0) };
  });
  const max = Math.max(1, ...counts.map((c) => c.n));
  return (
    <aside className="mq-side">
      <div className="a-side__head">
        <span className="a-eng">Source mix</span>
        <span className="a-eng">队列来源分布</span>
      </div>

      <div className="a-side__body">
        {counts.map((c) => {
          const s = SOURCES[c.id];
          return (
            <div className="a-mtr" key={c.id}>
              <div className="a-mtr__row">
                <span className="a-eng">
                  {s.channel} · {s.short}
                </span>
                <span className="a-mtr__v">
                  {c.n} 首 · {fmtLong(c.secs)}
                </span>
              </div>
              <div className="a-mtr__bar">
                <div
                  className={`a-mtr__fill ${c.id === "youtube" ? "is-b" : ""}`}
                  style={{ width: `${(c.n / max) * 100}%` }}
                />
              </div>
            </div>
          );
        })}

        <span className="a-side__label">
          <span className="a-eng">队列行为</span>
        </span>
        <div className="a-readout">
          <span className="a-eng">来源交替</span>
          <span className="a-readout__v is-amber">{mqAlternations(list)} 处</span>
        </div>
        <div className="a-readout">
          <span className="a-eng">跨源连续播放</span>
          <span className="a-readout__v is-green">开</span>
        </div>
        <div className="a-readout">
          <span className="a-eng">重复项</span>
          <span className="a-readout__v">已合并</span>
        </div>

        <div className="a-side__note" style={{ marginTop: 14 }}>
          队列是<b>跨来源</b>的，所以每一条都自带它自己的通道字母、音质与凭据。换来源时应用会
          重新解析一次直链 —— 这正是 <em>Track</em> 必须记住来源、而不是把来源当成当前页面状态的原因。
        </div>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// 存储
// ---------------------------------------------------------------------------

function MDiskPage() {
  const used = CACHE.totalMb;
  const limitMb = CACHE.limitMb;
  const pct = (v) => `${Math.max(1.5, (v / limitMb) * 100)}%`;
  return (
    <main className="a-list md">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>存储与离线</h1>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{CACHE.tracks}</b> 首已落盘
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              {fmtMb(used)} / {fmtMb(limitMb)}
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              命中率 <b>{Math.round(CACHE_POLICY.hitRate * 100)}%</b>
            </span>
          </div>
        </div>

        <div style={{ flex: "1 1 auto" }} />

        <span className="md__path" title={CACHE.folder}>
          <Icon name="folder" size={12} />
          <span>{CACHE.folder}</span>
        </span>
        <button className="a-iconbtn" title="复制路径">
          <Icon name="link" size={12} />
        </button>
        <button className="a-btn">更改</button>
      </div>

      <div className="md__meters">
        <div className="a-mtr">
          <div className="a-mtr__row">
            <span className="a-eng">总计</span>
            <span className="a-mtr__v is-amber">{fmtMb(used)}</span>
          </div>
          <div className="a-mtr__bar">
            <div className="a-mtr__fill" style={{ width: pct(used) }} />
          </div>
          <div className="a-mtr__scale">
            <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
              0
            </span>
            <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
              上限 {fmtMb(limitMb)}
            </span>
          </div>
        </div>

        {SOURCE_ORDER.map((id, i) => (
          <div className="a-mtr" key={id}>
            <div className="a-mtr__row">
              <span className="a-eng">
                {SOURCES[id].channel} · {SOURCES[id].short}
              </span>
              <span className="a-mtr__v">{fmtMb(CACHE.bySource[id])}</span>
            </div>
            <div className="a-mtr__bar">
              <div className={`a-mtr__fill ${i === 1 ? "is-b" : ""}`} style={{ width: `${(CACHE.bySource[id] / used) * 100}%` }} />
            </div>
            <div className="a-mtr__scale">
              <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
                {Math.round((CACHE.bySource[id] / used) * 100)}% 的缓存
              </span>
            </div>
          </div>
        ))}

        <div className="a-mtr">
          <div className="a-mtr__row">
            <span className="a-eng">歌词与封面</span>
            <span className="a-mtr__v">{fmtMb(CACHE.lyricsMb)}</span>
          </div>
          <div className="a-mtr__bar">
            <div className="a-mtr__fill is-c" style={{ width: `${(CACHE.lyricsMb / used) * 100}%` }} />
          </div>
          <div className="a-mtr__scale">
            <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
              {CACHE.partial} 项未完整
            </span>
          </div>
        </div>
      </div>

      <div className="a-cols md__cols">
        <span />
        <span className="a-eng">标题</span>
        <span className="a-eng">通道</span>
        <span className="a-eng">音质</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          大小
        </span>
        <span className="a-eng">状态</span>
        <span className="a-eng">最后播放</span>
        <span className="a-eng" style={{ textAlign: "right" }}>
          操作
        </span>
      </div>

      <div className="a-rows">
        {CACHE_ITEMS.map((it) => {
          const s = SOURCES[it.source];
          return (
            <div key={`${it.title}-${it.source}`} className={`md__row ${it.complete ? "" : "is-partial"}`}>
              <span
                className="a-cover a-cover--sm"
                style={{ background: "linear-gradient(142deg, #2a2a30, #14141a)" }}
              >
                {it.title.slice(0, 1)}
              </span>

              <div className="a-title">
                <div className="a-title__t" style={{ fontSize: 13 }}>
                  {it.title}
                </div>
                <div className="a-title__s" style={{ fontSize: 11 }}>
                  {it.variant}
                </div>
              </div>

              <div className="a-src">
                <span className="a-src__chip">
                  <b>{s.channel}</b>
                  <em>{s.short}</em>
                </span>
              </div>

              <span className="a-qual__t" style={{ fontSize: 10 }}>
                {it.quality}
              </span>

              <span className="md__size">{it.mb.toFixed(1)} MB</span>

              <span className="a-tag" style={it.complete ? undefined : { color: "var(--a-red)", borderColor: "rgba(242,99,95,.4)" }}>
                <span className={`a-led ${it.complete ? (it.pinned ? "is-on" : "is-ok") : "is-warn"}`} />
                {it.complete ? (it.pinned ? "已固定" : "完整") : "仅前半"}
              </span>

              <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
                {it.played}
              </span>

              <span className="md__acts">
                <button className="a-icobtn" title="固定，不参与淘汰">
                  <Icon name="lock" size={12} />
                </button>
                <button className="a-icobtn is-danger" title="删除本地副本">
                  <Icon name="close" size={12} />
                </button>
              </span>
            </div>
          );
        })}
      </div>

      <div className="mq__foot">
        <span className="a-eng">淘汰策略</span>
        <div className="a-seg">
          <button className="a-seg__cell is-on">LRU 最久未听</button>
          <button className="a-seg__cell">按来源清理</button>
          <button className="a-seg__cell">音频优先</button>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
          固定项与「正在播放」永不清理
        </span>
        <button className="a-btn">清理未固定</button>
      </div>
    </main>
  );
}

function MDiskSide() {
  const [rules, setRules] = React.useState(CACHE_POLICY.rules);
  const toggle = (id) => setRules(rules.map((r) => (r.id === id ? { ...r, on: !r.on } : r)));
  return (
    <aside className="md-side">
      <div className="a-side__head">
        <span className="a-eng">Store policy</span>
        <span className="a-eng">缓存策略</span>
      </div>

      <div className="a-side__body">
        <span className="a-side__label">
          <span className="a-eng">上限</span>
        </span>
        <div className="a-readout">
          <span className="a-eng">当前</span>
          <span className="a-readout__v is-amber">{fmtMb(CACHE.totalMb)}</span>
        </div>
        <div className="md__slider">
          <div className="md__slider__fill" style={{ width: `${(CACHE.totalMb / CACHE.limitMb) * 100}%` }} />
          <div className="md__slider__knob" style={{ left: `${(CACHE.totalMb / CACHE.limitMb) * 100}%` }} />
        </div>
        <div className="a-mtr__scale">
          <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
            2 GB
          </span>
          <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
            {fmtMb(CACHE.limitMb)}（{CACHE_POLICY.limitGb} GB）
          </span>
        </div>

        <span className="a-side__label">
          <span className="a-eng">规则</span>
        </span>
        {rules.map((r) => (
          <button
            key={r.id}
            className={`a-sw ${r.on ? "is-on" : ""}`}
            style={{ width: "100%", padding: "7px 0", textAlign: "left" }}
            onClick={() => toggle(r.id)}
          >
            <span className="a-sw__track">
              <i />
            </span>
            <span style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              <span style={{ fontSize: 11.5, color: "inherit" }}>{r.label}</span>
              <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)" }}>
                {r.detail}
              </span>
            </span>
          </button>
        ))}

        <span className="a-side__label">
          <span className="a-eng">本次会话</span>
        </span>
        <div className="a-readout">
          <span className="a-eng">新增</span>
          <span className="a-readout__v is-green">{CACHE_POLICY.addedThisSession} 首</span>
        </div>
        <div className="a-readout">
          <span className="a-eng">淘汰</span>
          <span className="a-readout__v">{CACHE_POLICY.evictedThisWeek} 首</span>
        </div>
        <div className="a-readout">
          <span className="a-eng">命中率</span>
          <span className="a-readout__v is-amber">{Math.round(CACHE_POLICY.hitRate * 100)}%</span>
        </div>
      </div>

      <div className="a-side__foot">
        <div className="a-side__note">
          只有字节数与 <b>Content-Length</b> 完全一致才算命中缓存。半成品在列表里被显式标成
          <em>「仅前半」</em>，而不是和完整项长得一样 —— 否则它会被当成完整歌曲播放并截断。
        </div>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// 输入（来源与会话）
// ---------------------------------------------------------------------------

const MI_SCOPES = [
  { id: "join", label: "参与聚合" },
  { id: "solo", label: "仅单独浏览" },
  { id: "off", label: "停用" },
];

const MI_SESSIONS = {
  bilibili: {
    kind: "account",
    name: "夜航西飞",
    detail: "大会员 · SESSDATA 有效至 11-02",
  },
  youtube: {
    kind: "anon",
    name: "匿名访问",
    detail: "visitorData 已取 · 大视频可完整播放",
  },
};

function MSourceModule({ id, caps, scope, setScope }) {
  const s = SOURCES[id];
  const on = caps.filter((c) => c.on).length;
  const sess = MI_SESSIONS[id];
  const isBili = id === "bilibili";
  const quals = isBili
    ? [
        { label: "无损 FLAC", note: "大会员 · 1411 kbps", on: true },
        { label: "192K AAC-LC", note: "登录后可用", on: true },
        { label: "132K / 64K", note: "匿名可用", on: true },
      ]
    : [
        { label: "AAC 130K", note: "itag 140 · 唯一可解码", on: true },
        { label: "AAC 50K", note: "itag 139 · HE-AAC", on: true },
        { label: "Opus", note: "itag 251 · 解码器不支持", on: false },
      ];
  return (
    <div className={`mi__module ${scope === "join" ? "is-live" : ""}`}>
      <div className="mi__head">
        <span className="mi__grip" title="拖动排序">
          <Icon name="queue" size={12} />
        </span>
        <span className="mi__letter">{s.channel}</span>
        <span className="mi__name">{s.name}</span>
        <span className={`a-led ${scope === "off" ? "is-idle" : "is-ok"}`} />
        <span className="a-eng">{MI_SCOPES.find((x) => x.id === scope).label}</span>
      </div>

      <div className="mi__body">
        <div className="mi__col">
          <span className="a-eng">会话</span>
          <div className="mi__sess" style={{ marginTop: 7 }}>
            <span className={`mi__avatar ${sess.kind === "anon" ? "is-anon" : ""}`}>
              {sess.kind === "anon" ? <Icon name="users" size={13} /> : sess.name.slice(0, 1)}
            </span>
            <span style={{ display: "flex", flexDirection: "column", gap: 3, minWidth: 0 }}>
              <span style={{ fontSize: 12, color: "var(--a-fg)" }}>{sess.name}</span>
              <span className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg4)" }}>
                {sess.detail}
              </span>
            </span>
          </div>
          <div className="mi__acts">
            {sess.kind === "account" ? (
              <>
                <button className="a-btn">重新扫码</button>
                <button className="a-btn">刷新凭据</button>
                <button className="a-btn">退出</button>
              </>
            ) : (
              <>
                <button className="a-btn">刷新 visitorData</button>
                <button className="a-btn">改用账号登录</button>
              </>
            )}
          </div>
        </div>

        <div className="mi__col">
          <span className="a-eng">能力声明 · {on} / {caps.length}</span>
          <div className="mi__caps" style={{ marginTop: 7 }}>
            {caps.map((c) => (
              <span key={c.id} className={`cap ${c.on ? "" : "is-off"}`} title={c.on ? "已声明" : "未声明"}>
                <i />
                {c.label}
              </span>
            ))}
          </div>
          {caps.some((c) => !c.on) ? (
            <div className="a-mono" style={{ fontSize: 9.5, color: "var(--a-fg4)", marginTop: 9, lineHeight: 1.6 }}>
              未声明的 {caps.filter((c) => !c.on).length} 项（
              {caps.filter((c) => !c.on).map((c) => c.label).join(" / ")}）不会在界面上出现 ——
              不是空的页签，是直接没有。
            </div>
          ) : null}
        </div>

        <div className="mi__col">
          <span className="a-eng">音质优先级</span>
          <div className="mi__qual" style={{ marginTop: 7 }}>
            {quals.map((q, i) => (
              <span key={q.label} className={`mi__qrow ${q.on ? "" : "is-off"}`}>
                <span className="mi__qrank">{i + 1}</span>
                <span style={{ color: q.on ? "var(--a-fg)" : undefined }}>{q.label}</span>
                <span className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)", marginLeft: "auto" }}>
                  {q.note}
                </span>
              </span>
            ))}
          </div>
        </div>
      </div>

      <div className="mi__foot">
        <span className="a-eng">作用范围</span>
        <div className="a-seg">
          {MI_SCOPES.map((x) => (
            <button
              key={x.id}
              className={`a-seg__cell ${scope === x.id ? "is-on" : ""}`}
              onClick={() => setScope(x.id)}
            >
              {x.label}
            </button>
          ))}
        </div>
        <div className="mi__data">
          <span className="a-readout" style={{ gap: 8 }}>
            <span className="a-eng">缓存</span>
            <span className="a-readout__v">{fmtMb(CACHE.bySource[id])}</span>
          </span>
          <span className="a-readout" style={{ gap: 8 }}>
            <span className="a-eng">队列</span>
            <span className="a-readout__v">{QUEUE.filter((q) => q.source === id).length} 条</span>
          </span>
          <span className="a-readout" style={{ gap: 8 }}>
            <span className="a-eng">收藏夹</span>
            <span className="a-readout__v">{isBili ? "128 首" : "未声明"}</span>
          </span>
        </div>
      </div>
    </div>
  );
}

function MSourcePage({ onConnect }) {
  const [scopes, setScopes] = React.useState(SOURCE_PREFS.scopes);
  const setScope = (id, v) => setScopes({ ...scopes, [id]: v });
  const joining = SOURCE_ORDER.filter((id) => scopes[id] === "join").length;
  return (
    <main className="a-list mi">
      <div className="a-list__head">
        <div>
          <div className="a-list__q">
            <h1>来源与会话</h1>
          </div>
          <div className="a-list__meta">
            <span className="a-list__m">
              <b>{SOURCE_ORDER.length}</b> 个来源
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">
              参与聚合 <b>{joining}</b> 个
            </span>
            <span className="a-list__sep" />
            <span className="a-list__m">2 个在线</span>
          </div>
        </div>

        <div style={{ flex: "1 1 auto" }} />

        <span className="a-eng">默认来源</span>
        <div className="a-seg">
          {SOURCE_ORDER.map((id) => (
            <button key={id} className={`a-seg__cell ${SOURCE_PREFS.defaultSource === id ? "is-on" : ""}`}>
              {SOURCES[id].short}
            </button>
          ))}
        </div>

        <button className="a-btn is-primary" onClick={onConnect}>
          <Icon name="plus" size={11} />
          接入来源
        </button>
      </div>

      <div className="a-rows">
        <div className="a-note" style={{ marginBottom: 6 }}>
          <Icon name="plug" size={14} style={{ color: "var(--a-amber)", flex: "none" }} />
          <span className="a-note__t">
            一个来源就是<b>一份能力声明</b>。声明了什么，界面就长出什么；没声明的品目直接不出现，
            而不是显示成一个空房间 —— 这就是第二个来源今天没有收藏夹页签的原因。
          </span>
        </div>

        <MSourceModule
          id="bilibili"
          caps={CAPS_BILIBILI}
          scope={scopes.bilibili}
          setScope={(v) => setScope("bilibili", v)}
        />
        <MSourceModule
          id="youtube"
          caps={CAPS_YOUTUBE}
          scope={scopes.youtube}
          setScope={(v) => setScope("youtube", v)}
        />
      </div>
    </main>
  );
}

function MSourceSide({ onConnect }) {
  return (
    <aside className="mi-side">
      <div className="a-side__head">
        <span className="a-eng">Contract</span>
        <span className="a-eng">声明项 → 长出的界面</span>
      </div>

      <div className="a-side__body">
        {CAP_GROUPS.map((g) => (
          <div key={g.group} style={{ marginBottom: 10 }}>
            <span className="a-eng">{g.group}</span>
            <div style={{ marginTop: 5 }}>
              {g.items.map((it) => {
                const inA = CAPS_BILIBILI.find((c) => c.id === it.id)?.on;
                const inB = CAPS_YOUTUBE.find((c) => c.id === it.id)?.on;
                return (
                  <div className="caprow" key={it.id}>
                    <span>{it.label}</span>
                    <span className="caprow__s">{it.surface}</span>
                    <span className={`caprow__d ${inA ? "is-on" : ""}`} title={inA ? "B 站已声明" : "B 站未声明"} />
                    <span className={`caprow__d ${inB ? "is-on" : ""}`} title={inB ? "YouTube 已声明" : "YouTube 未声明"} />
                  </div>
                );
              })}
            </div>
          </div>
        ))}
        <div className="a-mono" style={{ fontSize: 9, color: "var(--a-fg4)", marginTop: 4 }}>
          两个圆点依次代表 B 站 / YouTube 是否声明了该项
        </div>
      </div>

      <div className="a-side__foot">
        <button className="a-btn is-primary" style={{ width: "100%", justifyContent: "center" }} onClick={onConnect}>
          <Icon name="plus" size={11} />
          {CONNECTOR.label}
        </button>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// 接入来源 — the one flow that takes the whole surface
// ---------------------------------------------------------------------------

function MConnectSheet({ onClose }) {
  const [name, setName] = React.useState("");
  const [letter, setLetter] = React.useState("C");
  const [session, setSession] = React.useState("anon");
  const [caps, setCaps] = React.useState(
    () => new Set(["search", "paging", "stream", "quality", "anon", "captions"])
  );
  const toggle = (id) =>
    setCaps((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const all = CAP_GROUPS.flatMap((g) => g.items);
  const chosen = all.filter((it) => caps.has(it.id));
  const missing = all.length - chosen.length;
  const taken = ["A", "B"].includes(letter.trim().toUpperCase());

  return (
    <div className="mx-scrim" onClick={onClose}>
      <div className="mx-sheet" onClick={(e) => e.stopPropagation()}>
        <div className="mx__head">
          <span className="a-brand__bars">
            <Icon name="plug" size={16} />
          </span>
          <h2>接入新来源</h2>
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            写入一份能力声明，界面据此长出对应品目
          </span>
          <div style={{ flex: "1 1 auto" }} />
          <button className="a-iconbtn" onClick={onClose} title="关闭">
            <Icon name="close" size={13} />
          </button>
        </div>

        <div className="mx__body">
          <div className="mx__step">
            <div className="mx__steplbl">
              <span className="mx__n">01</span>
              <span className="a-eng">基本信息</span>
            </div>
            <div className="mx__fields">
              <div className="mx__field">
                <label>名称</label>
                <input
                  className="mx__input"
                  style={{ width: 240 }}
                  placeholder="例如：某个音乐平台"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </div>
              <div className="mx__field">
                <label>通道字母</label>
                <input
                  className="mx__input is-mono"
                  maxLength={1}
                  value={letter}
                  onChange={(e) => setLetter(e.target.value.toUpperCase())}
                />
              </div>
              <div className="mx__field" style={{ justifyContent: "flex-end", paddingBottom: 6 }}>
                <span className={`mx__hint ${taken ? "is-warn" : ""}`}>
                  {taken ? `通道 ${letter} 已被占用` : `通道 ${letter} 可用 · 列表里会显示为 ${letter}·条目`}
                </span>
              </div>
            </div>
          </div>

          <div className="mx__step">
            <div className="mx__steplbl">
              <span className="mx__n">02</span>
              <span className="a-eng">能力声明</span>
            </div>
            <div>
              <div className="mx__caps">
                {CAP_GROUPS.map((g) => (
                  <div className="mx__group" key={g.group}>
                    <span className="a-eng">{g.group}</span>
                    {g.items.map((it) => (
                      <button
                        key={it.id}
                        className={`mx__cap ${caps.has(it.id) ? "is-on" : ""}`}
                        onClick={() => toggle(it.id)}
                      >
                        <span className="mx__box">
                          <i />
                        </span>
                        <span className="mx__cap__t">{it.label}</span>
                        <span className="mx__cap__why">{caps.has(it.id) ? "会启用" : "不启用"}</span>
                      </button>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="mx__step">
            <div className="mx__steplbl">
              <span className="mx__n">03</span>
              <span className="a-eng">会话方式</span>
            </div>
            <div>
              <div className="mx__radio">
                <div className="a-seg">
                  {[
                    { id: "anon", label: "匿名 / 无需登录" },
                    { id: "qr", label: "扫码" },
                    { id: "oauth", label: "OAuth" },
                    { id: "cookie", label: "Cookie 导入" },
                  ].map((x) => (
                    <button
                      key={x.id}
                      className={`a-seg__cell ${session === x.id ? "is-on" : ""}`}
                      onClick={() => setSession(x.id)}
                    >
                      {x.label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="mx__hint" style={{ marginTop: 8 }}>
                {session === "anon"
                  ? "匿名可用时不需要登录；凭据以域名隔离存放，与其它来源互不可见"
                  : "凭据会按域名隔离加密落盘，与其它来源的 Cookie 互不可见"}
              </div>
            </div>
          </div>

          <div className="mx__readout">
            <div className="mx__readout__t">
              这份声明（<b>{chosen.length}</b> 项）会让界面长出上面列出的品目；
              未声明的 <b>{missing}</b> 项不会出现在任何地方。
            </div>
            <div className="mx__surfaces">
              {chosen.map((it) => (
                <span className="a-tag" key={it.id}>
                  <span className="a-led is-ok" />
                  {it.surface}
                </span>
              ))}
              {chosen.length === 0 ? (
                <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
                  一项都没声明 —— 这个来源接进来也什么都做不了
                </span>
              ) : null}
            </div>
          </div>
        </div>

        <div className="mx__foot">
          <Icon name="filter" size={13} style={{ color: "var(--a-fg4)" }} />
          <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg4)" }}>
            接入后会先跑一次校验：搜索一页、解析一条直链、取一份文本
          </span>
          <div style={{ flex: "1 1 auto" }} />
          <button className="a-btn" onClick={onClose}>
            取消
          </button>
          <button className="a-btn is-primary">
            <Icon name="check" size={11} />
            接入并校验
          </button>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, {
  MQueuePage,
  MQueueSide,
  MDiskPage,
  MDiskSide,
  MSourcePage,
  MSourceSide,
  MConnectSheet,
});

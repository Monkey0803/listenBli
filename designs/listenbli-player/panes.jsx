// panes.jsx — presentational components: cover art, scrubber, top bar, track
// list, lyrics panel, player bar, status bar. Props in, callbacks out.

const { useState, useRef, useEffect, useLayoutEffect, useCallback, useMemo } = React;

const CONFIG_PATH = "~/Library/Application Support/listenBli/config.json";

// -- atoms ------------------------------------------------------------------

function Cover({ track, size = 46, className = "" }) {
  const glyphSize = Math.round(size * 0.44);
  if (!track) {
    return (
      <div
        className={`cover ${className}`}
        style={{ width: size, height: size, background: "linear-gradient(148deg, #23232c, #16161d)" }}
      >
        <span className="cover__glyph" style={{ fontSize: glyphSize, opacity: 0.5 }}>♪</span>
      </div>
    );
  }
  return (
    <div
      className={`cover ${className}`}
      style={{
        width: size,
        height: size,
        background: `linear-gradient(148deg, hsl(${track.h1} 58% 45%), hsl(${track.h2} 54% 21%))`,
      }}
    >
      <span className="cover__glyph" style={{ fontSize: glyphSize }}>{track.initial || "♪"}</span>
    </div>
  );
}

function Equalizer({ playing = true, small = false }) {
  return (
    <span className={`eq ${playing ? "" : "eq--paused"} ${small ? "eq--sm" : ""}`} aria-hidden="true">
      <i style={{ height: "60%" }}></i>
      <i style={{ height: "100%" }}></i>
      <i style={{ height: "76%" }}></i>
      <i style={{ height: "44%" }}></i>
    </span>
  );
}

function QualityBadge({ quality }) {
  if (!quality) return <span className="badge badge--dim">解析中</span>;
  return <span className={`badge badge--${quality.tone}`}>{quality.short}</span>;
}

/** Draggable bar used for both the seek bar and the volume slider. */
function Scrub({ value, onChange, buffered = 0, className = "", ariaLabel }) {
  const ref = useRef(null);
  const [dragging, setDragging] = useState(false);

  const posFromEvent = (clientX) => {
    const rect = ref.current.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  useEffect(() => {
    if (!dragging) return;
    const move = (e) => onChange(posFromEvent(e.clientX));
    const up = () => setDragging(false);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, [dragging, onChange]);

  const pct = Math.min(100, Math.max(0, value * 100));

  return (
    <div
      ref={ref}
      className={`scrub ${dragging ? "scrub--active" : ""} ${className}`}
      role="slider"
      aria-label={ariaLabel}
      aria-valuenow={Math.round(pct)}
      aria-valuemin="0"
      aria-valuemax="100"
      tabIndex={0}
      onPointerDown={(e) => {
        e.preventDefault();
        setDragging(true);
        onChange(posFromEvent(e.clientX));
      }}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") onChange(Math.max(0, value - 0.02));
        if (e.key === "ArrowRight") onChange(Math.min(1, value + 0.02));
      }}
    >
      <div className="scrub__track">
        <div className="scrub__buf" style={{ width: `${Math.min(100, buffered * 100)}%` }}></div>
        <div className="scrub__fill" style={{ width: `${pct}%` }}></div>
      </div>
      <div className="scrub__knob" style={{ left: `${pct}%` }}></div>
    </div>
  );
}

function Switch({ checked, onChange, label }) {
  return (
    <label className="switch">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className="switch__track"><span className="switch__knob"></span></span>
      {label ? <span className="switch__label">{label}</span> : null}
    </label>
  );
}

// -- top bar ----------------------------------------------------------------

function TopBar({
  tab, setTab, keyword, setKeyword, onSearch, searching,
  loggedIn, user, preferFlac, setPreferFlac,
  onOpenSettings, onOpenLogin, onLogout, settingsOpen,
  favCount, historyCount, playing,
}) {
  const [menuOpen, setMenuOpen] = useState(false);
  const searchRef = useRef(null);

  useEffect(() => {
    const onKey = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current && searchRef.current.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuOpen(false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [menuOpen]);

  return (
    <header className="topbar">
      <div className={`brand ${playing ? "brand--playing" : ""}`}>
        <span className="brand__mark">
          <span className="brand__bar"></span>
          <span className="brand__bar"></span>
          <span className="brand__bar"></span>
        </span>
        <span className="brand__word">listen<em>Bli</em></span>
      </div>

      <nav className="segmented" role="tablist" aria-label="内容分类">
        <button
          role="tab"
          aria-selected={tab === "search"}
          className={`seg ${tab === "search" ? "seg--active" : ""}`}
          onClick={() => setTab("search")}
        >
          <IconSearch size={13} />
          搜索
          {searching ? <span className="spinning"><span className="spin" style={{ width: 11, height: 11, borderWidth: 1.5 }}></span></span> : null}
        </button>
        <button
          role="tab"
          aria-selected={tab === "favorites"}
          className={`seg ${tab === "favorites" ? "seg--active" : ""}`}
          onClick={() => setTab("favorites")}
        >
          {loggedIn ? <IconFolder size={13} /> : <IconLock size={13} className="seg__lock" />}
          收藏夹
          {loggedIn ? <span className="seg__count">{favCount}</span> : null}
        </button>
        <button
          role="tab"
          aria-selected={tab === "history"}
          className={`seg ${tab === "history" ? "seg--active" : ""}`}
          onClick={() => setTab("history")}
        >
          {loggedIn ? <IconClock size={13} /> : <IconLock size={13} className="seg__lock" />}
          历史
          {loggedIn ? <span className="seg__count">{historyCount}</span> : null}
        </button>
      </nav>

      <form
        className="search"
        onSubmit={(e) => { e.preventDefault(); onSearch(); }}
      >
        <IconSearch size={15} className="search__icon" />
        <input
          ref={searchRef}
          value={keyword}
          onChange={(e) => setKeyword(e.target.value)}
          placeholder="搜索歌曲 / 视频，如：周杰伦 晴天"
          aria-label="搜索"
          spellCheck="false"
        />
        {keyword ? (
          <button type="button" className="search__clear" onClick={() => setKeyword("")} aria-label="清空搜索">
            <IconClose size={13} />
          </button>
        ) : (
          <span className="search__key">⌘K</span>
        )}
      </form>

      <div className="topbar__spacer"></div>

      <div className="topbar__right">
        <button
          className={`chip ${preferFlac ? "chip--on" : ""}`}
          onClick={() => setPreferFlac(!preferFlac)}
          title={preferFlac ? "已开启无损优先（需大会员）" : "开启无损优先"}
        >
          <IconSpark size={12} className="chip__dot" />
          无损优先
        </button>

        <button
          className={`iconbtn ${settingsOpen ? "iconbtn--on" : ""}`}
          onClick={onOpenSettings}
          aria-label="设置"
          title="设置"
        >
          <IconSettings size={17} />
        </button>

        {loggedIn ? (
          <div style={{ position: "relative" }}>
            <button className="account" onClick={(e) => { e.stopPropagation(); setMenuOpen(!menuOpen); }}>
              <span className="account__avatar">{user.uname.slice(0, 1)}</span>
              <span className="account__name">{user.uname}</span>
              {user.vip ? <span className="account__vip">大会员</span> : null}
              <IconChevron size={13} style={{ color: "var(--fg-4)" }} />
            </button>
            {menuOpen ? (
              <div className="menu" onClick={(e) => e.stopPropagation()}>
                <div className="menu__head">
                  <div className="menu__name">{user.uname}</div>
                  <div className="menu__mid">UID {user.mid}</div>
                </div>
                <button className="menu__item" onClick={onOpenSettings}>
                  <IconSettings size={14} />
                  播放设置
                </button>
                <button className="menu__item menu__item--danger" onClick={() => { setMenuOpen(false); onLogout(); }}>
                  <IconClose size={14} />
                  退出登录
                </button>
              </div>
            ) : null}
          </div>
        ) : (
          <button className="chip" onClick={onOpenLogin} style={{ height: 30, padding: "0 12px" }}>
            <IconQr size={13} />
            扫码登录
          </button>
        )}
      </div>
    </header>
  );
}

// -- track list -------------------------------------------------------------

function TrackRow({ track, index, current, playing, resolved, listKey, onPlay }) {
  return (
    <button
      className={`row ${current ? "row--current" : ""}`}
      style={{ "--i": index % 14 }}
      onClick={onPlay}
      aria-current={current ? "true" : undefined}
    >
      <span className="row__idx">{String(index + 1).padStart(2, "0")}</span>

      <span className="row__cover">
        <Cover track={track} size={46} />
        <span className="row__hoverplay" aria-hidden="true">
          {current && playing ? <Equalizer small /> : <IconPlay size={15} />}
        </span>
      </span>

      <span className="row__text">
        <span className="row__title" title={track.title}>{track.title}</span>
        <span className="row__sub">
          <b>{track.author || "未知作者"}</b>
          <i></i>
          {track.restricted ? (
            <span className="badge badge--warn" style={{ height: 16, padding: "0 5px" }}>大会员专享</span>
          ) : track.noLyrics ? (
            <span>无歌词</span>
          ) : (
            <span>{track.flac ? "支持无损" : "192K"}</span>
          )}
        </span>
      </span>

      <span className="row__right">
        <span className="row__badgeslot">
          {current ? <QualityBadge quality={resolved} /> : null}
        </span>
        <span className="row__dur mono">{fmtTime(track.duration)}</span>
        <span className="row__more" aria-hidden="true"><IconMore size={16} /></span>
      </span>
    </button>
  );
}

function EmptyState({ icon, title, desc, children }) {
  return (
    <div className="state">
      <div className="state__art">{icon}</div>
      <div className="state__title">{title}</div>
      {desc ? <div className="state__desc">{desc}</div> : null}
      {children}
    </div>
  );
}

function ListPane({
  keyword, searching, results, tab, loggedIn, currentKey, playing, resolvedQuality, onPlay, onPlayAll,
  onOpenLogin, favFolders, selectedFolder, setSelectedFolder, favItems, favLoading, favHasMore, onLoadMore,
  onRefreshFav, history, historyLoading, onRefreshHistory, favCount, onSuggest,
}) {
  if (tab === "favorites" && !loggedIn) {
    return (
      <section className="listpane" data-screen-label="收藏夹-未登录">
        <div className="listhead">
          <div className="listhead__title">
            <span className="listhead__kw">收藏夹</span>
          </div>
        </div>
        <EmptyState
          icon={<IconLock size={28} />}
          title="登录后可查看收藏夹"
          desc="listenBli 未登录也能正常听歌，但要读取「我的」内容需要一次扫码。凭据会加密保存在本机，重启后依然有效。"
        >
          <button className="btn-primary" onClick={onOpenLogin} style={{ marginTop: 6 }}>
            <IconQr size={15} />
            扫码登录
          </button>
        </EmptyState>
      </section>
    );
  }

  if (tab === "history" && !loggedIn) {
    return (
      <section className="listpane" data-screen-label="历史-未登录">
        <div className="listhead">
          <div className="listhead__title">
            <span className="listhead__kw">历史</span>
          </div>
        </div>
        <EmptyState
          icon={<IconClock size={28} />}
          title="登录后可查看观看历史"
          desc="历史记录只包含视频类内容，音频投稿不会出现在这里。"
        >
          <button className="btn-primary" onClick={onOpenLogin} style={{ marginTop: 6 }}>
            <IconQr size={15} />
            扫码登录
          </button>
        </EmptyState>
      </section>
    );
  }

  if (tab === "favorites") {
    return (
      <section className="listpane" data-screen-label="收藏夹">
        <div className="listhead">
          <div className="listhead__title">
            <span className="listhead__kw">收藏夹</span>
            <span className="listhead__meta">
              {favItems.length ? `${favItems.length} 首已载入` : "正在载入"}
            </span>
          </div>
          <div className="listhead__spacer"></div>
          <button className="btn-ghost" onClick={onRefreshFav}>
            <IconRefresh size={13} />
            刷新
          </button>
        </div>

        <div className="folderbar">
          <span className="folderbar__label">文件夹</span>
          <div className="folderbar__scroll">
            {favFolders.map((folder) => (
              <button
                key={folder.id}
                className={`folder ${folder.id === selectedFolder ? "folder--active" : ""}`}
                onClick={() => setSelectedFolder(folder.id)}
              >
                <IconFolder size={13} />
                {folder.title}
                <span className="folder__n">{folder.media_count}</span>
              </button>
            ))}
          </div>
        </div>

        {favLoading ? (
          <div className="skeleton">
            {[0, 1, 2, 3, 4].map((i) => <div key={i} className="sk"></div>)}
          </div>
        ) : favItems.length === 0 ? (
          <EmptyState
            icon={<IconFolder size={26} />}
            title="这个收藏夹里没有可播放的视频"
            desc="收藏夹里的图文、番剧和付费内容无法提取音轨，因此不会出现在这里。"
          />
        ) : (
          <div className="scroll" style={{ flex: 1, minHeight: 0 }}>
            <div className="colhead">
              <span>标题</span>
              <span className="colhead__dur">音质 / 时长</span>
            </div>
            <div className="rowlist">
              {favItems.map((track, i) => (
                <TrackRow
                  key={track.bvid + i}
                  track={track}
                  index={i}
                  current={currentKey === track.bvid}
                  playing={playing}
                  resolved={currentKey === track.bvid ? resolvedQuality : null}
                  onPlay={() => onPlay(i)}
                />
              ))}
            </div>
            {favHasMore ? (
              <div style={{ display: "flex", justifyContent: "center", padding: "4px 0 26px" }}>
                <button className="btn-ghost" onClick={onLoadMore} disabled={favLoading}>
                  <IconChevron size={13} />
                  加载更多
                </button>
              </div>
            ) : null}
          </div>
        )}
      </section>
    );
  }

  if (tab === "history") {
    return (
      <section className="listpane" data-screen-label="历史">
        <div className="listhead">
          <div className="listhead__title">
            <span className="listhead__kw">历史</span>
            <span className="listhead__meta">{history.length} 条记录</span>
          </div>
          <div className="listhead__spacer"></div>
          <button className="btn-ghost" onClick={onRefreshHistory}>
            <IconRefresh size={13} />
            刷新历史
          </button>
        </div>
        {historyLoading ? (
          <div className="skeleton">
            {[0, 1, 2, 3, 4].map((i) => <div key={i} className="sk"></div>)}
          </div>
        ) : (
          <div className="scroll" style={{ flex: 1, minHeight: 0 }}>
            <div className="colhead">
              <span>标题</span>
              <span className="colhead__dur">音质 / 时长</span>
            </div>
            <div className="rowlist">
              {history.map((track, i) => (
                <TrackRow
                  key={track.bvid + i}
                  track={track}
                  index={i}
                  current={currentKey === track.bvid}
                  playing={playing}
                  resolved={currentKey === track.bvid ? resolvedQuality : null}
                  onPlay={() => onPlay(i)}
                />
              ))}
            </div>
          </div>
        )}
      </section>
    );
  }

  // search
  return (
    <section className="listpane" data-screen-label="搜索">
      <div className="listhead">
        <div className="listhead__title">
          <span className="listhead__kw">{searching ? `搜索「${keyword}」` : keyword ? `「${keyword}」` : "搜索"}</span>
          {!searching && results.length ? <span className="listhead__meta">找到 {results.length} 个结果</span> : null}
          {searching ? <span className="spin" style={{ marginLeft: 2 }}></span> : null}
        </div>
        {!searching && results.length ? (
          <>
            <div className="listhead__spacer"></div>
            <button className="btn-ghost btn-ghost--accent" onClick={onPlayAll}>
              <IconPlay size={12} />
              全部播放
            </button>
          </>
        ) : null}
      </div>

      {searching ? (
        <div className="skeleton">
          {[0, 1, 2, 3, 4, 5].map((i) => <div key={i} className="sk" style={{ animationDelay: `${i * 60}ms` }}></div>)}
        </div>
      ) : results.length === 0 ? (
        <EmptyState
          icon={<IconSearch size={28} />}
          title={keyword ? `没有找到「${keyword}」相关的结果` : "输入关键词开始搜索"}
          desc={
            keyword
              ? "试试换一个关键词，或者用「歌手 + 歌名」的形式，例如「周杰伦 晴天」或「米津玄師 Lemon」。"
              : "listenBli 会把 B 站上的音乐/视频下载成音轨后直接播放，不需要登录即可获得 192K 音质。"
          }
        >
          <div className="state__keys">
            {["周杰伦 晴天", "米津玄師", "七里香"].map((s) => (
              <button key={s} className="chip" onClick={() => onSuggest && onSuggest(s)}>{s}</button>
            ))}
          </div>
        </EmptyState>
      ) : (
        <div className="scroll" style={{ flex: 1, minHeight: 0 }}>
          <div className="colhead">
            <span>标题</span>
            <span className="colhead__dur">音质 / 时长</span>
          </div>
          <div className="rowlist">
            {results.map((track, i) => (
              <TrackRow
                key={track.bvid + i}
                track={track}
                index={i}
                current={currentKey === track.bvid}
                playing={playing}
                resolved={currentKey === track.bvid ? resolvedQuality : null}
                onPlay={() => onPlay(i)}
              />
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

// -- lyrics -----------------------------------------------------------------

function LyricsPanel({
  track, lyrics, lyricsState, position, follow, setFollow, showTr, setShowTr, onSeek, onManualScroll, hue,
}) {
  const bodyRef = useRef(null);
  const lineRefs = useRef([]);
  const scrollFallback = useRef(0);
  const [expired, setExpired] = useState(false);

  const currentIndex = useMemo(() => {
    if (!lyrics || !lyrics.lines) return null;
    let idx = null;
    for (let i = 0; i < lyrics.lines.length; i += 1) {
      if (lyrics.lines[i].time <= position) idx = i;
      else break;
    }
    return idx;
  }, [lyrics, position]);

  // Centre the active line ourselves rather than scrollIntoView: it keeps
  // working under the scaled stage transform. Smooth scrolling is silently
  // skipped in some compositor-less contexts (headless, offscreen), which would
  // leave the line stranded off-centre — so commit the position as a fallback.
  useLayoutEffect(() => {
    if (!follow || currentIndex == null) return;
    const el = lineRefs.current[currentIndex];
    const body = bodyRef.current;
    if (!el || !body) return;
    const target = Math.max(0, el.offsetTop - body.clientHeight / 2 + el.offsetHeight / 2);
    const from = body.scrollTop;
    body.scrollTo({ top: target, behavior: "smooth" });
    clearTimeout(scrollFallback.current);
    scrollFallback.current = setTimeout(() => {
      if (Math.abs(body.scrollTop - from) < 2 && Math.abs(target - from) > 4) {
        body.scrollTop = target;
      }
    }, 420);
    return () => clearTimeout(scrollFallback.current);
  }, [currentIndex, follow]);

  useEffect(() => {
    if (lyricsState !== "ready") return;
    setExpired(false);
    const timer = setTimeout(() => setExpired(true), 9000);
    return () => clearTimeout(timer);
  }, [lyricsState, track && track.bvid]);

  const src = lyrics ? lyrics.source : "none";

  return (
    <aside className="lyrics" data-screen-label="歌词面板">
      <div
        className="lyrics__bloom"
        style={{ "--bh": (hue && hue.h1) || 340, "--bh2": (hue && hue.h2) || 280, opacity: track ? 0.28 : 0.1 }}
      ></div>

      <div className="lyrics__head">
        <IconLyrics size={15} style={{ color: "var(--fg-3)" }} />
        <span className="lyrics__title">歌词</span>
        <div className="lyrics__spacer"></div>
        <div className="lyrics__toggles">
          <button
            className={`minitoggle ${follow ? "minitoggle--on" : ""}`}
            onClick={() => setFollow(!follow)}
            title="自动高亮并居中当前歌词行"
          >
            <IconTarget size={12} />
            跟随
          </button>
          <button
            className={`minitoggle ${showTr ? "minitoggle--on" : ""}`}
            onClick={() => setShowTr(!showTr)}
            title="显示翻译"
          >
            <IconTranslate size={12} />
            翻译
          </button>
        </div>
      </div>

      <div className="lyrics__meta">
        <span>来源</span>
        <span className={`lyrics__src ${src === "none" ? "lyrics__src--none" : ""}`}>
          {sourceLabel(src)}
        </span>
        {lyrics && lyrics.lines ? <span>· {lyrics.lines.length} 行</span> : null}
        {lyricsState === "ready" && expired ? <span style={{ color: "var(--warn)" }}>· 已缓存</span> : null}
      </div>

      {lyricsState === "idle" ? (
        <div className="lyrics__body lyrics__body--idle">
          <EmptyState
            icon={<IconLyrics size={26} />}
            title="还没有在播放"
            desc="从左侧选一首开始，歌词会自动跟随高亮、居中滚动。点击任意一行可以跳到那一句。"
          />
        </div>
      ) : lyricsState === "loading" ? (
        <div className="lyr-shimmer" aria-label="歌词加载中">
          <span style={{ width: "58%" }}></span>
          <span style={{ width: "76%" }}></span>
          <span style={{ width: "44%" }}></span>
          <span style={{ width: "68%" }}></span>
          <span style={{ width: "52%" }}></span>
        </div>
      ) : lyricsState === "none" ? (
        <div className="lyrics__body lyrics__body--idle">
          <EmptyState
            icon={<IconTranslate size={26} />}
            title="暂无歌词"
            desc="这个视频没有 CC 字幕，也没能在网易云匹配到可用的歌词。合集与纯音乐类投稿通常都没有。"
          />
        </div>
      ) : (
        <div className="lyrics__body scroll" ref={bodyRef} onWheel={onManualScroll}>
          {lyrics.lines.map((line, i) => {
            const isCurrent = i === currentIndex;
            const distance = currentIndex == null ? 99 : Math.abs(i - currentIndex);
            const cls = [
              "lyr-line",
              isCurrent ? "lyr-line--current" : "",
              !isCurrent && distance <= 2 ? "lyr-line--near" : "",
              currentIndex != null && i < currentIndex ? "lyr-line--past" : "",
            ].join(" ");
            return (
              <button
                key={i}
                ref={(el) => { lineRefs.current[i] = el; }}
                className={cls}
                onClick={() => onSeek(line.time)}
                title={`跳到 ${fmtTime(line.time)}`}
              >
                {line.text}
                {showTr && line.tr ? <span className="lyr-line__tr">{line.tr}</span> : null}
              </button>
            );
          })}
        </div>
      )}

      {!follow && lyricsState === "ready" ? (
        <button className="lyr-resume" onClick={() => setFollow(true)}>
          <span className="lyr-resume__dot"></span>
          跟随已暂停 · 点击恢复
        </button>
      ) : null}
    </aside>
  );
}

// -- player bar -------------------------------------------------------------

function PlayerBar({
  current, quality, playing, loading, progress, position, duration, buffered,
  volume, setVolume, onTogglePlay, onPrev, onNext, onSeek,
  queue, queuePos, onJump, hue, download, engineError,
}) {
  const [queueOpen, setQueueOpen] = useState(false);
  const muted = volume === 0;
  const queueLabel = queue.length ? `播放列表 ${queuePos + 1}/${queue.length}` : "播放列表为空";

  return (
    <section className="player" data-screen-label="播放条">
      <div className="player__bloom" style={{ "--bh": (hue && hue.h1) || 340, "--bh2": (hue && hue.h2) || 280, opacity: current ? 0.24 : 0.06 }}></div>

      <div className="player__row">
        <div className={`nowplaying ${current ? "" : "nowplaying--empty"}`}>
          <Cover track={current} size={54} />
          <div className="nowplaying__text">
            {current ? (
              <>
                <div className="nowplaying__title" title={current.title}>{current.title}</div>
                <div className="nowplaying__sub">
                  <b>{current.author || "未知作者"}</b>
                  <i style={{ width: 3, height: 3, borderRadius: "50%", background: "var(--fg-4)", display: "inline-block" }}></i>
                  {loading ? <span>下载中…</span> : <span>{quality ? quality.label : "未知音质"}</span>}
                  {current.flac && quality && quality.id === "flac" ? <IconSpark size={11} style={{ color: "var(--gold)" }} /> : null}
                </div>
              </>
            ) : (
              <>
                <div className="nowplaying__title" style={{ color: "var(--fg-3)", fontWeight: 500 }}>未在播放</div>
                <div className="nowplaying__sub">从左侧列表中选择一首开始</div>
              </>
            )}
          </div>
        </div>

        <div className="transport">
          <div className="transport__controls">
            <button className="tbtn" onClick={onPrev} disabled={!queue.length} title="上一首">
              <IconPrev size={17} />
            </button>
            <button
              className={`playbtn ${playing ? "playbtn--playing" : ""}`}
              onClick={onTogglePlay}
              disabled={!current || loading}
              title={playing ? "暂停" : "播放"}
            >
              {loading ? <span className="spin" style={{ borderTopColor: "#2a0d18", borderColor: "rgba(42,13,24,.25)", borderTopWidth: 2 }}></span>
                : playing ? <IconPause size={19} /> : <IconPlay size={19} />}
            </button>
            <button className="tbtn" onClick={onNext} disabled={!queue.length} title="下一首">
              <IconNext size={17} />
            </button>
          </div>

          <div className="seekrow">
            <span className="seekrow__t">{fmtTime(position)}</span>
            <Scrub
              value={progress}
              buffered={buffered}
              onChange={onSeek}
              ariaLabel="播放进度"
            />
            <span className="seekrow__t seekrow__t--end">{fmtTime(duration || (current ? current.duration : 0))}</span>
          </div>
        </div>

        <div className="pb-right">
          <div style={{ position: "relative" }}>
            <button
              className={`chip ${queueOpen ? "chip--on" : ""}`}
              onClick={() => setQueueOpen(!queueOpen)}
              title="展开播放列表"
            >
              <IconQueue size={13} />
              {queueLabel}
            </button>
            {queueOpen ? (
              <div className="queue-pop">
                <div className="queue-pop__head">
                  <IconQueue size={14} style={{ color: "var(--accent)" }} />
                  <span style={{ fontSize: 13, fontWeight: 600 }}>播放列表</span>
                  <span className="mono" style={{ fontSize: 11, color: "var(--fg-4)" }}>{queue.length} 首</span>
                  <div style={{ flex: 1 }}></div>
                  <button className="iconbtn" onClick={() => setQueueOpen(false)} aria-label="收起">
                    <IconClose size={14} />
                  </button>
                </div>
                <div className="queue-pop__list scroll">
                  {queue.map((t, i) => (
                    <button
                      key={t.bvid + i}
                      className={`qrow ${i === queuePos ? "qrow--current" : ""}`}
                      onClick={() => onJump(i)}
                    >
                      <span className="qrow__n">{i === queuePos ? "▶" : String(i + 1).padStart(2, "0")}</span>
                      <span className="qrow__t">{t.title}</span>
                      <span className="qrow__d">{fmtTime(t.duration)}</span>
                    </button>
                  ))}
                </div>
              </div>
            ) : null}
          </div>

          <div className="volwrap">
            <button
              className="iconbtn"
              onClick={() => setVolume(muted ? 0.8 : 0)}
              title={muted ? "取消静音" : "静音"}
              style={{ width: 28, height: 28 }}
            >
              <IconVolume size={16} muted={muted} />
            </button>
            <Scrub
              className="scrub--vol"
              value={volume}
              onChange={setVolume}
              ariaLabel="音量"
            />
          </div>
        </div>
      </div>

      <div className="player__sub">
        {download ? (
          <div className="dlbar">
            <span className="dlbar__track">
              <span className="dlbar__fill" style={{ width: `${download.ratio * 100}%` }}></span>
            </span>
            <span className="dlbar__text">
              缓存音频 {Math.round(download.ratio * 100)}% · {fmtBytes(download.got)}
              {download.total ? ` / ${fmtBytes(download.total)}` : ""}
            </span>
          </div>
        ) : (
          <span className="hintline">
            {current
              ? "空格 播放 / 暂停 · ← → 快退快进 5 秒 · 点击歌词行跳转"
              : "音频会先缓存到本地再播放，通常 1–7 MB"}
          </span>
        )}
        <div style={{ flex: 1 }}></div>
        {engineError ? <span className="statusbar__err">音频输出不可用</span> : null}
      </div>
    </section>
  );
}

// -- status bar -------------------------------------------------------------

function StatusBar({ status, loggedIn, engineError, onCopy, toast }) {
  return (
    <footer className="statusbar" data-screen-label="状态栏">
      {status ? (
        <span className="status__msg">
          <span className={`status__dot ${status.isError ? "status__dot--err" : "status__dot--ok"}`}></span>
          <span style={{ color: status.isError ? "var(--err)" : "var(--fg-2)" }}>{status.text}</span>
        </span>
      ) : (
        <span className="status__msg" style={{ color: "var(--fg-3)" }}>
          <span className="status__dot" style={{ background: loggedIn ? "var(--ok)" : "var(--fg-4)" }}></span>
          <span>{loggedIn ? "已登录 · 无损优先可用" : "未登录（可正常听歌；登录后可获取更高音质与「我的」内容）"}</span>
        </span>
      )}

      <div className="statusbar__spacer"></div>

      {engineError ? <span className="statusbar__err">{engineError}</span> : null}

      <span className="statusbar__path">
        配置：{CONFIG_PATH}
        <button onClick={onCopy} title="复制配置路径" aria-label="复制配置路径">
          <IconCopy size={12} />
        </button>
      </span>

      {toast ? (
        <span className="toast" style={{ bottom: 44, left: "50%" }}>
          <IconCheck size={14} />
          {toast}
        </span>
      ) : null}
    </footer>
  );
}

Object.assign(window, {
  Cover,
  Equalizer,
  QualityBadge,
  Scrub,
  Switch,
  TopBar,
  TrackRow,
  EmptyState,
  ListPane,
  LyricsPanel,
  PlayerBar,
  StatusBar,
  CONFIG_PATH,
});

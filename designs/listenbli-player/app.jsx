// app.jsx — the orchestrator. Owns every piece of app state and the playback
// clock, and mounts the shell. State lives here and flows down as props; the
// presentational files never hold app state.

const { useState, useRef, useEffect, useCallback, useMemo } = React;

const SIZE_OF_LIST = (track) => Math.round((1.6 + (track.duration / 269) * 3.1) * 1024 * 1024);
const STAT_TTL = 6000;
const FOLLOW_SUSPEND = 3000;

// Deterministic favourites so switching folders feels real without inventing
// a backend.
const folderItems = (folders, folderId) => {
  const idx = Math.max(0, folders.findIndex((f) => f.id === folderId));
  const pool = [].concat(window.JAY_TRACKS, window.LEMON_TRACKS, window.HISTORY_TRACKS);
  const offset = idx * 3;
  return Array.from({ length: idx === 0 ? 7 : 5 + idx }, (_, i) => pool[(offset + i) % pool.length]);
};

function App() {
  // -- navigation / search ------------------------------------------------
  const [tab, setTab] = useState("search");
  const [keyword, setKeyword] = useState("周杰伦");
  const [lastKeyword, setLastKeyword] = useState("周杰伦");
  const [searching, setSearching] = useState(false);
  const [results, setResults] = useState(window.JAY_TRACKS);

  // -- playback -----------------------------------------------------------
  const [queue, setQueue] = useState(window.JAY_TRACKS);
  const [queuePos, setQueuePos] = useState(0);
  const [current, setCurrent] = useState(null);
  const [playing, setPlaying] = useState(false);
  const [loading, setLoading] = useState(false);
  const [position, setPosition] = useState(0);
  const [quality, setQuality] = useState(null);
  const [download, setDownload] = useState(null);
  const [volume, setVolume] = useState(0.8);

  // -- lyrics -------------------------------------------------------------
  const [lyrics, setLyrics] = useState(null);
  const [lyricsState, setLyricsState] = useState("idle");
  const [follow, setFollow] = useState(true);
  const [showTr, setShowTr] = useState(true);
  const manualAt = useRef(0);

  // -- session / library --------------------------------------------------
  const [user, setUser] = useState(null);
  const [qrState, setQrState] = useState(null);
  const [favFolders, setFavFolders] = useState([]);
  const [selectedFolder, setSelectedFolder] = useState(null);
  const [favItems, setFavItems] = useState([]);
  const [favLoading, setFavLoading] = useState(false);
  const [favHasMore, setFavHasMore] = useState(false);
  const [history, setHistory] = useState([]);
  const [historyLoading, setHistoryLoading] = useState(false);

  // -- preferences / chrome ----------------------------------------------
  const [preferFlac, setPreferFlac] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [accent, setAccent] = useState("pink");
  const [lyricSize, setLyricSize] = useState(15.5);
  const [density, setDensity] = useState("comfortable");
  const [cjkFont, setCjkFont] = useState("auto");
  const [cjkPath, setCjkPath] = useState("");
  const [status, setStatus] = useState(null);
  const [toast, setToast] = useState(null);

  const timers = useRef([]);
  const later = useCallback((fn, ms) => {
    const id = setTimeout(fn, ms);
    timers.current.push(id);
    return id;
  }, []);
  useEffect(() => () => timers.current.forEach(clearTimeout), []);

  const say = useCallback((text, isError = false) => {
    setStatus({ text, isError, at: Date.now() });
  }, []);

  const duration = current ? current.duration : 0;

  // -- design tokens driven by the settings controls ----------------------
  useEffect(() => {
    const a = ACCENTS.find((x) => x.id === accent) || ACCENTS[0];
    const root = document.documentElement;
    root.style.setProperty("--accent", a.accent);
    root.style.setProperty("--accent-deep", a.deep);
    root.style.setProperty("--accent-soft", a.soft);
    root.style.setProperty("--accent-ink", a.ink);
    root.style.setProperty("--accent-glow", `color-mix(in oklab, ${a.accent} 55%, transparent)`);
    root.style.setProperty("--accent-dim", `color-mix(in oklab, ${a.accent} 22%, transparent)`);
  }, [accent]);

  useEffect(() => {
    document.documentElement.style.setProperty("--lyric-size", `${lyricSize}px`);
  }, [lyricSize]);

  useEffect(() => {
    document.documentElement.style.setProperty("--row-h", density === "compact" ? "52px" : "62px");
    document.getElementById("root").style.setProperty("--row-h", density === "compact" ? "52px" : "62px");
  }, [density]);

  // -- starting a track ---------------------------------------------------
  const stopDownload = useCallback(() => setDownload(null), []);

  const playTrack = useCallback((track, list, index) => {
    setQueue(list);
    setQueuePos(index);
    setCurrent(track);
    setPosition(0);
    setQuality(null);
    setLyrics(null);
    setFollow(true);

    if (track.restricted) {
      setPlaying(false);
      setLoading(false);
      stopDownload();
      setLyricsState("none");
      say("解析音频失败：该视频需大会员账号或已下架", true);
      return;
    }

    // The real app downloads the audio to a local cache before it plays —
    // show that honestly, with the downloaded bytes and the seek-bar buffer.
    setLyricsState("loading");
    setLoading(true);
    setPlaying(false);

    const total = SIZE_OF_LIST(track);
    let ratio = 0;
    setDownload({ ratio: 0, got: 0, total });

    const step = () => {
      ratio = Math.min(1, ratio + 0.045 + Math.random() * 0.05);
      setDownload({ ratio, got: Math.round(total * ratio), total });
      if (ratio < 1) {
        later(step, 70);
        return;
      }
      stopDownload();
      setLoading(false);
      setPlaying(true);

      const flac = preferFlac && user && user.vip && track.flac;
      setQuality(flac ? window.QUALITY.flac : window.QUALITY.k192);

      const found = window.LYRICS[track.bvid];
      if (track.noLyrics || !found) {
        later(() => setLyricsState("none"), 500);
      } else {
        later(() => {
          setLyrics(found);
          setLyricsState("ready");
        }, 620);
      }
    };
    later(step, 90);
  }, [later, preferFlac, user, say, stopDownload]);

  // First paint: land on a real song rather than an empty shell.
  useEffect(() => {
    const t = setTimeout(() => playTrack(window.JAY_TRACKS[0], window.JAY_TRACKS, 0), 420);
    const seek = setTimeout(() => setPosition(96), 1500);
    return () => { clearTimeout(t); clearTimeout(seek); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // -- playback clock -----------------------------------------------------
  useEffect(() => {
    if (!playing || loading || !current) return undefined;
    const id = setInterval(() => {
      setPosition((p) => {
        const next = p + 0.2;
        if (next >= duration) {
          // Auto-advance, but stop at the end of the queue instead of looping
          // back to the top (an explicit "next" still wraps).
          const len = queue.length;
          const nextPos = (queuePos + 1) % len;
          if (len > 1 && nextPos === 0 && queuePos + 1 >= len) {
            setPlaying(false);
            return duration;
          }
          later(() => playTrack(queue[nextPos], queue, nextPos), 0);
          return duration;
        }
        return next;
      });
    }, 200);
    return () => clearInterval(id);
  }, [playing, loading, current, duration, queue, queuePos, playTrack, later]);

  // -- status TTL ---------------------------------------------------------
  useEffect(() => {
    if (!status) return undefined;
    const id = setTimeout(() => setStatus(null), STAT_TTL);
    return () => clearTimeout(id);
  }, [status]);

  // -- follow resumes 3s after the last manual scroll ---------------------
  useEffect(() => {
    if (follow) return undefined;
    const id = setInterval(() => {
      if (manualAt.current && Date.now() - manualAt.current >= FOLLOW_SUSPEND) {
        setFollow(true);
      }
    }, 400);
    return () => clearInterval(id);
  }, [follow]);

  // -- actions ------------------------------------------------------------
  const onSearch = useCallback((override) => {
    const kw = (override !== undefined ? override : keyword).trim();
    if (!kw) { say("请输入关键词", false); return; }
    setTab("search");
    setSearching(true);
    setLastKeyword(kw);
    setResults([]);
    later(() => {
      const found = window.searchTracks(kw) || [];
      setSearching(false);
      setResults(found);
      if (found.length === 0) say("没有找到结果", false);
    }, 680);
  }, [keyword, later, say]);

  const playFromList = useCallback((list, index) => {
    const track = list[index];
    if (track) playTrack(track, list, index);
  }, [playTrack]);

  const next = useCallback((userInitiated = true) => {
    if (!queue.length) return;
    const len = queue.length;
    const nextPos = (queuePos + 1) % len;
    if (!userInitiated && len > 1 && nextPos === 0) {
      setPlaying(false);
      setPosition(duration);
      return;
    }
    playTrack(queue[nextPos], queue, nextPos);
  }, [queue, queuePos, duration, playTrack]);

  const prev = useCallback(() => {
    if (!queue.length) return;
    const prevPos = queuePos === 0 ? queue.length - 1 : queuePos - 1;
    playTrack(queue[prevPos], queue, prevPos);
  }, [queue, queuePos, playTrack]);

  const togglePlay = useCallback(() => {
    if (!current || loading) return;
    setPlaying((p) => !p);
  }, [current, loading]);

  const seekFraction = useCallback((f) => {
    setPosition(Math.max(0, Math.min(duration, f * duration)));
  }, [duration]);

  const seekToTime = useCallback((time) => {
    setPosition(time);
    setPlaying(true);
    manualAt.current = 0;
    setFollow(true);
  }, []);

  const onManualScroll = useCallback(() => {
    manualAt.current = Date.now();
    setFollow(false);
  }, []);

  const toggleFlac = useCallback((on) => {
    setPreferFlac(on);
    if (on) {
      say(user && user.vip
        ? "已开启无损优先（需投稿提供 FLAC，否则回落 192K）"
        : "已开启无损优先：登录大会员账号后才会生效", false);
    } else {
      say("已关闭无损优先", false);
    }
  }, [say, user]);

  // -- login -------------------------------------------------------------
  const openLogin = useCallback(() => {
    setSettingsOpen(false);
    setQrState("waiting");
  }, []);

  const finishLogin = useCallback(() => {
    setUser(window.USER);
    setQrState(null);
    const folders = window.FAV_FOLDERS;
    setFavFolders(folders);
    setSelectedFolder(folders[0].id);
    setFavItems(folderItems(folders, folders[0].id));
    setFavHasMore(true);
    setHistory(window.HISTORY_TRACKS);
    setHistoryLoading(false);
    say("已登录 · 音质与「我的」内容已解锁", false);
  }, [say]);

  const scanQr = useCallback(() => {
    setQrState("scanned");
    later(finishLogin, 1700);
  }, [later, finishLogin]);

  const logout = useCallback(() => {
    setUser(null);
    setFavFolders([]);
    setFavItems([]);
    setSelectedFolder(null);
    setHistory([]);
    say("已退出登录，凭据已从本机清除", false);
  }, [say]);

  // -- library -----------------------------------------------------------
  const pickFolder = useCallback((id) => {
    setSelectedFolder(id);
    setFavItems([]);
    setFavLoading(true);
    later(() => {
      setFavItems(folderItems(window.FAV_FOLDERS, id));
      setFavHasMore(true);
      setFavLoading(false);
    }, 520);
  }, [later]);

  const loadMore = useCallback(() => {
    setFavLoading(true);
    later(() => {
      const pool = [].concat(window.JAY_TRACKS, window.HISTORY_TRACKS);
      setFavItems((items) => items.concat(pool.slice(0, 4)));
      setFavHasMore(false);
      setFavLoading(false);
    }, 620);
  }, [later]);

  const refreshFav = useCallback(() => {
    setFavLoading(true);
    later(() => {
      setFavItems(folderItems(window.FAV_FOLDERS, selectedFolder));
      setFavLoading(false);
      say("收藏夹已刷新", false);
    }, 560);
  }, [later, selectedFolder, say]);

  const refreshHistory = useCallback(() => {
    setHistoryLoading(true);
    setHistory([]);
    later(() => {
      setHistory(window.HISTORY_TRACKS);
      setHistoryLoading(false);
      say("历史记录已刷新", false);
    }, 560);
  }, [later, say]);

  // -- clipboard ---------------------------------------------------------
  const copyPath = useCallback((value) => {
    const text = value || "~/Library/Application Support/listenBli/config.json";
    if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
    setToast("已复制到剪贴板");
    later(() => setToast(null), 1600);
  }, [later]);

  // -- keyboard ----------------------------------------------------------
  useEffect(() => {
    const onKey = (e) => {
      const tag = (e.target && e.target.tagName) || "";
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      if (e.key === "Escape") {
        setQrState(null);
        setSettingsOpen(false);
        return;
      }
      if (e.code === "Space") { e.preventDefault(); togglePlay(); }
      if (e.key === "ArrowLeft") setPosition((p) => Math.max(0, p - 5));
      if (e.key === "ArrowRight") setPosition((p) => Math.min(duration, p + 5));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePlay, duration]);

  // -- derived -----------------------------------------------------------
  const progress = duration ? Math.min(1, position / duration) : 0;
  const buffered = download ? download.ratio : (loading ? 0 : 1);
  const hue = current || { h1: 340, h2: 280 };
  const resolvedQuality = quality;

  const folderCounts = favFolders.reduce(
    (acc, f) => (f.id === selectedFolder ? f.media_count : acc),
    0
  );

  return (
    <>
      <TopBar
        tab={tab}
        setTab={setTab}
        keyword={keyword}
        setKeyword={setKeyword}
        onSearch={onSearch}
        searching={searching}
        loggedIn={!!user}
        user={user || window.USER}
        preferFlac={preferFlac}
        setPreferFlac={toggleFlac}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenLogin={openLogin}
        onLogout={logout}
        settingsOpen={settingsOpen}
        favCount={favFolders.length ? folderCounts : 0}
        historyCount={history.length}
        playing={playing}
      />

      <div className="main">
        <ListPane
          keyword={lastKeyword}
          searching={searching}
          results={results}
          tab={tab}
          loggedIn={!!user}
          currentKey={current ? current.bvid : null}
          playing={playing}
          resolvedQuality={resolvedQuality}
          onPlay={(i) => playFromList(results, i)}
          onPlayAll={() => results.length && playFromList(results, 0)}
          onOpenLogin={openLogin}
          favFolders={favFolders}
          selectedFolder={selectedFolder}
          setSelectedFolder={pickFolder}
          favItems={favItems}
          favLoading={favLoading}
          favHasMore={favHasMore}
          onLoadMore={loadMore}
          onRefreshFav={refreshFav}
          history={history}
          historyLoading={historyLoading}
          onRefreshHistory={refreshHistory}
          favCount={folderCounts}
          onSuggest={(s) => { setKeyword(s); onSearch(s); }}
        />

        <LyricsPanel
          track={current}
          lyrics={lyrics}
          lyricsState={lyricsState}
          position={position}
          follow={follow}
          setFollow={setFollow}
          showTr={showTr}
          setShowTr={setShowTr}
          onSeek={seekToTime}
          onManualScroll={onManualScroll}
          hue={hue}
        />
      </div>

      <PlayerBar
        current={current}
        quality={quality}
        playing={playing}
        loading={loading}
        progress={progress}
        position={position}
        duration={duration}
        buffered={buffered}
        volume={volume}
        setVolume={setVolume}
        onTogglePlay={togglePlay}
        onPrev={prev}
        onNext={() => next(true)}
        onSeek={seekFraction}
        queue={queue}
        queuePos={queuePos}
        onJump={(i) => playTrack(queue[i], queue, i)}
        hue={hue}
        download={download}
        engineError={null}
      />

      <StatusBar
        status={status}
        loggedIn={!!user}
        engineError={null}
        onCopy={() => copyPath()}
        toast={toast}
      />

      {qrState ? (
        <LoginModal
          state={qrState}
          onScan={scanQr}
          onRefresh={() => setQrState("waiting")}
          onClose={() => setQrState(null)}
          onSimulateExpired={() => setQrState("expired")}
        />
      ) : null}

      {settingsOpen ? (
        <SettingsSheet
          onClose={() => setSettingsOpen(false)}
          preferFlac={preferFlac}
          setPreferFlac={toggleFlac}
          showTr={showTr}
          setShowTr={setShowTr}
          quality={quality}
          loggedIn={!!user}
          accent={accent}
          setAccent={setAccent}
          lyricSize={lyricSize}
          setLyricSize={setLyricSize}
          density={density}
          setDensity={setDensity}
          cjkFont={cjkFont}
          setCjkFont={setCjkFont}
          cjkPath={cjkPath}
          setCjkPath={setCjkPath}
          onCopyPath={() => copyPath(cjkPath || "/System/Library/Fonts/PingFang.ttc")}
        />
      ) : null}
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);

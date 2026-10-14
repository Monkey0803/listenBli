// library-app.jsx — the shell for the 听歌 re-cut.
//
// Direction A's console, with a different centre: the left rail is 库导航, the
// right column is 正在播放 (cover + song facts + lyrics), and the platform is gone
// from the surface entirely — the only way to a platform is the 来源 button.
//
// URL:  ?page=library|playlists|recent|offline|search|sources
//       ?playlist=<id>   ?sheet=source
// Keys: 1–5 switch pages, Esc closes the source sheet.

const LIB_PARAMS = new URLSearchParams(location.search);
const LIB_START = LIB_PARAMS.get("page") || "library";
const LIB_KEYS = ["library", "playlists", "recent", "offline", "search"];

function LTopBar({ onSearch, onSources }) {
  return (
    <header className="a-top">
      <div className="a-brand">
        <span className="a-brand__bars">
          <Icon name="wave" size={20} />
        </span>
        <span className="a-brand__word">
          listen<i>Bli</i>
        </span>
        <span className="a-brand__tag">Library · 我的音乐</span>
      </div>

      <button className="a-search" onClick={onSearch} style={{ textAlign: "left" }}>
        <Icon name="search" size={13} />
        <span style={{ flex: "1 1 auto", fontSize: 13, color: "var(--a-fg3)" }}>搜索歌曲、歌手、专辑</span>
        <span className="a-search__key">⌘K</span>
      </button>

      <div style={{ flex: "1 1 auto" }} />

      <button className="a-chip" onClick={onSources}>
        <Icon name="plug" size={13} />
        <span className="a-eng">Sync</span>
        <span className="a-mono" style={{ fontSize: 10, color: "var(--a-green)" }}>
          刚刚
        </span>
      </button>

      <button className="a-chip" onClick={onSources}>
        <Icon name="download" size={13} />
        <span className="a-eng">Offline</span>
        <span className="a-mono" style={{ fontSize: 10, color: "var(--a-fg3)" }}>
          8 首
        </span>
      </button>

      <button className="a-iconbtn" title="设置">
        <Icon name="settings" size={14} />
      </button>

      <button className="a-account" title="账号">
        <span className="a-account__av">夜</span>
        <span className="a-account__t">
          <span className="a-account__n">夜航西飞</span>
          <span className="a-account__r">本地库</span>
        </span>
      </button>
    </header>
  );
}

function LibraryApp() {
  const [page, setPage] = React.useState(LIB_START);
  const [current, setCurrent] = React.useState(NOW.entryId);
  const [playlist, setPlaylist] = React.useState(LIB_PARAMS.get("playlist") || null);
  const [sourceSheet, setSourceSheet] = React.useState(LIB_PARAMS.get("sheet") === "source");
  const [connectOpen, setConnectOpen] = React.useState(LIB_PARAMS.get("sheet") === "connect");

  const song = LIBRARY.find((s) => s.id === current) || LIBRARY[0];

  // Clicking a song anywhere makes it the thing that is playing. That is the one
  // interaction this whole layout is built around.
  const play = (id) => {
    setCurrent(id);
  };

  React.useEffect(() => {
    const onKey = (e) => {
      if (e.key === "Escape") {
        setSourceSheet(false);
        setConnectOpen(false);
        return;
      }
      if (e.target && /input|textarea/i.test(e.target.tagName)) return;
      const i = "12345".indexOf(e.key);
      if (i >= 0) setPage(LIB_KEYS[i]);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="dira" data-screen-label={`Library · ${page}`}>
      <LTopBar onSearch={() => setPage("search")} onSources={() => setPage("sources")} />
      <LNav page={page} onPage={setPage} onSources={() => setPage("sources")} onConnect={() => setConnectOpen(true)} />

      {page === "library" ? (
        <LLibraryPage current={current} setCurrent={play} onSource={() => setPage("sources")} />
      ) : null}
      {page === "playlists" ? (
        <LPlaylistPage open={playlist} setOpen={setPlaylist} current={current} setCurrent={play} />
      ) : null}
      {page === "recent" ? <LRecentPage current={current} setCurrent={play} /> : null}
      {page === "offline" ? <LOfflinePage current={current} setCurrent={play} /> : null}
      {page === "search" ? <LSearchPage current={current} setCurrent={play} /> : null}
      {page === "sources" ? <LSourcesPage onConnect={() => setConnectOpen(true)} /> : null}

      <LNowPlaying song={song} onSource={() => setSourceSheet(true)} />
      <LDeck song={song} onSource={() => setSourceSheet(true)} />

      {sourceSheet ? <LSourceSheet song={song} onClose={() => setSourceSheet(false)} /> : null}
      {connectOpen ? <MConnectSheet onClose={() => setConnectOpen(false)} /> : null}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<LibraryApp />);

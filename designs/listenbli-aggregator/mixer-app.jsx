// mixer-app.jsx — the deep prototype's shell.
//
// Direction A, made whole: the same top bar, input rail and transport deck, with
// four extra pages behind the desk's page keys. Nothing here forks the aggregate
// page — it composes the components direction-a.jsx already owns, so the two
// cannot drift apart.
//
// URL:  ?page=mix|queue|disk|input   ?scope=all|bilibili|youtube   ?merge=off
//       ?sheet=connect
// Keys: 1–4 switch pages, Esc closes the connect sheet.

const MIX_PARAMS = new URLSearchParams(location.search);
const MIX_START_PAGE = MIX_PARAMS.get("page") || "mix";
const MIX_START_SCOPE = MIX_PARAMS.get("scope") || "all";

function MixerApp() {
  const [page, setPage] = React.useState(MIX_START_PAGE);
  const [scope, setScope] = React.useState(MIX_START_SCOPE);
  const [merged, setMerged] = React.useState(initialMerged());
  const [connect, setConnect] = React.useState(MIX_PARAMS.get("sheet") === "connect");

  // Page keys on a real desk are under your fingers, so they are here too.
  React.useEffect(() => {
    const onKey = (e) => {
      if (e.key === "Escape") {
        setConnect(false);
        return;
      }
      if (e.target && /input|textarea/i.test(e.target.tagName)) return;
      const index = "1234".indexOf(e.key);
      if (index >= 0) setPage(A_PAGES[index].id);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const openSource = (id) => {
    setScope(id);
    setPage("mix");
  };

  return (
    <div className="dira" data-screen-label={`A · ${page}`}>
      <ATopBar page={page} onPage={setPage} scope={scope} onScope={openSource} />
      <AInputRail page={page} onPage={setPage} onConnect={() => setConnect(true)} />

      {page === "mix" ? (
        <>
          <AResultList
            merged={merged}
            setMerged={setMerged}
            currentId={NOW.entryId}
            scope={scope}
            onOpenQueue={() => setPage("queue")}
          />
          <ALyricFeed currentId={NOW.entryId} />
        </>
      ) : null}

      {page === "queue" ? (
        <>
          <MQueuePage onOpenMix={() => setPage("mix")} />
          <MQueueSide />
        </>
      ) : null}

      {page === "disk" ? (
        <>
          <MDiskPage />
          <MDiskSide />
        </>
      ) : null}

      {page === "input" ? (
        <>
          <MSourcePage onConnect={() => setConnect(true)} />
          <MSourceSide onConnect={() => setConnect(true)} />
        </>
      ) : null}

      <ADeck currentId={NOW.entryId} />

      {connect ? <MConnectSheet onClose={() => setConnect(false)} /> : null}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<MixerApp />);

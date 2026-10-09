// overlays.jsx — the two things that sit above the app surface: the QR login
// modal and the settings sheet. The settings sheet also hosts the in-design
// controls (accent colour, lyric size, list density) so directions are easy to
// explore without leaving the prototype.

const { useState, useMemo } = React;

// -- QR matrix --------------------------------------------------------------
// A deterministic, QR-looking module grid: real finder + timing patterns so it
// reads as a Bilibili login code, filled with seeded noise. Not a scannable
// symbol — the real app renders the bytes from the API.

function buildQrMatrix(seed, size = 25) {
  let state = 0;
  for (let i = 0; i < seed.length; i += 1) {
    state = (state * 31 + seed.charCodeAt(i)) >>> 0;
  }
  const rand = () => {
    state ^= state << 13; state >>>= 0;
    state ^= state >> 17;
    state ^= state << 5; state >>>= 0;
    return state / 4294967296;
  };

  const finder = (r, c, top, left) => {
    const dr = r - top;
    const dc = c - left;
    if (dr < 0 || dr > 6 || dc < 0 || dc > 6) return null;
    const ring = dr === 0 || dr === 6 || dc === 0 || dc === 6;
    const core = dr >= 2 && dr <= 4 && dc >= 2 && dc <= 4;
    return ring || core;
  };

  const cells = [];
  for (let r = 0; r < size; r += 1) {
    for (let c = 0; c < size; c += 1) {
      // keep a 5×5 quiet block under the centred logo
      const centre = r >= 10 && r <= 14 && c >= 10 && c <= 14;
      if (centre) { cells.push(false); continue; }

      const tl = finder(r, c, 0, 0);
      const tr = finder(r, c, 0, size - 7);
      const bl = finder(r, c, size - 7, 0);
      const sep =
        (r <= 7 && c <= 7) || (r <= 7 && c >= size - 8) || (r >= size - 8 && c <= 7);

      if (tl !== null) { cells.push(tl); continue; }
      if (tr !== null) { cells.push(tr); continue; }
      if (bl !== null) { cells.push(bl); continue; }
      if (sep) { cells.push(false); continue; }
      if (r === 6) { cells.push(c % 2 === 0); continue; }
      if (c === 6) { cells.push(r % 2 === 0); continue; }
      if (((r + c) % 17 === 0) || ((r * 3 + c * 5) % 23 === 0)) { cells.push(true); continue; }
      cells.push(rand() > 0.49);
    }
  }
  return cells;
}

function QrCode({ seed, state }) {
  const cells = useMemo(() => buildQrMatrix(seed, 25), [seed]);

  return (
    <div className={`qrwrap ${state !== "waiting" ? "qrwrap--done" : ""} ${state === "expired" ? "qrwrap--expired" : ""}`}>
      <div className="qrgrid">
        {cells.map((on, i) => (
          <i key={i} className={on ? "" : "off"}></i>
        ))}
      </div>
      <div className="qrlogo">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M8 6.2c0-1 1.1-1.6 1.9-1.1l8.6 5.3c.8.5.8 1.7 0 2.2l-8.6 5.3c-.8.5-1.9-.1-1.9-1.1Z" fill="currentColor" />
        </svg>
      </div>
      {state === "waiting" ? <div className="qrscan"></div> : null}
      {state === "scanned" ? (
        <div className="qrveil">
          <span className="spin" style={{ width: 26, height: 26, borderWidth: 2.4, position: "absolute", top: "34%" }}></span>
          <span className="qrveil__ring" style={{ opacity: 0 }}>
            <IconCheck size={24} />
          </span>
          <span className="qrveil__t">已扫码</span>
          <span className="qrveil__s">请在手机上确认登录</span>
        </div>
      ) : null}
      {state === "done" ? (
        <div className="qrveil">
          <span className="qrveil__ring"><IconCheck size={26} /></span>
          <span className="qrveil__t">登录成功</span>
          <span className="qrveil__s">正在同步收藏夹与历史…</span>
        </div>
      ) : null}
      {state === "expired" ? (
        <div className="qrveil">
          <span className="qrveil__ring" style={{ background: "linear-gradient(145deg,#6b7280,#4b5563)" }}>
            <IconRefresh size={24} />
          </span>
          <span className="qrveil__t">二维码已过期</span>
          <span className="qrveil__s">点击刷新后重新扫码</span>
        </div>
      ) : null}
    </div>
  );
}

// -- login modal ------------------------------------------------------------

function LoginModal({ state, onScan, onRefresh, onClose, onSimulateExpired }) {
  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} data-screen-label="扫码登录">
        <div className="modal__title">扫码登录</div>
        <div className="modal__desc">
          使用哔哩哔哩手机客户端扫码，凭据会加密保存在本机，重启后仍然有效。
        </div>

        <div onClick={state === "waiting" ? onScan : state === "expired" ? onRefresh : undefined}>
          <QrCode seed={state === "expired" ? "expired-key-refresh" : "listenBli-demo-qr"} state={state} />
        </div>

        <div className="modal__foot">
          {state === "expired" ? (
            <button className="btn-primary" onClick={onRefresh}>
              <IconRefresh size={15} />
              刷新二维码
            </button>
          ) : (
            <button className="btn-ghost" onClick={onRefresh} disabled={state !== "waiting"}>
              <IconRefresh size={13} />
              刷新二维码
            </button>
          )}
          <button className="btn-ghost" onClick={onClose}>关闭</button>
        </div>

        <div className="modal__tips">
          <div className="tip">
            <IconSpark size={14} />
            <span>登录后解锁 <b style={{ color: "var(--fg-2)" }}>FLAC 无损</b>、「我的收藏夹」与观看历史。</span>
          </div>
          <div className="tip">
            <IconLock size={14} />
            <span><b style={{ color: "var(--fg-2)" }}>SESSDATA</b> 等凭据只会写入本机配置文件，权限 0600。</span>
          </div>
        </div>

        <div className="modal__sim">
          这是设计稿的模拟扫码 —
          <button onClick={state === "waiting" ? onScan : onRefresh}>
            {state === "waiting" ? "点击二维码" : "点此刷新"}
          </button>
          ·
          <button onClick={onSimulateExpired}>模拟二维码过期</button>
        </div>
      </div>
    </div>
  );
}

// -- settings sheet ---------------------------------------------------------

const ACCENTS = [
  { id: "pink", name: "信号粉", accent: "#fb7299", deep: "#d9578a", soft: "#ff9db8", ink: "#ffc8d8" },
  { id: "cyan", name: "霓虹青", accent: "#35b6e4", deep: "#1d86ad", soft: "#7fd6f0", ink: "#b7e9f8" },
  { id: "violet", name: "夜紫", accent: "#a78bfa", deep: "#7c5cf0", soft: "#c4b0fd", ink: "#ddd2fe" },
];

const CJK_FONTS = [
  { id: "auto", name: "自动探测" },
  { id: "pingfang", name: "PingFang SC" },
  { id: "hiragino", name: "Hiragino Sans GB" },
  { id: "stheiti", name: "STHeiti" },
  { id: "songti", name: "Songti SC" },
];

function SettingsSheet({
  onClose, preferFlac, setPreferFlac, showTr, setShowTr, quality, loggedIn,
  accent, setAccent, lyricSize, setLyricSize, density, setDensity,
  cjkFont, setCjkFont, cjkPath, setCjkPath, onCopyPath,
}) {
  return (
    <div className="scrim" style={{ placeItems: "stretch end", background: "rgba(4,4,7,.5)" }} onClick={onClose}>
      <div className="sheet" onClick={(e) => e.stopPropagation()} data-screen-label="设置">
        <div className="sheet__head">
          <IconSettings size={17} style={{ color: "var(--accent)" }} />
          <span className="sheet__title">设置</span>
          <div style={{ flex: 1 }}></div>
          <button className="iconbtn" onClick={onClose} aria-label="关闭设置">
            <IconClose size={16} />
          </button>
        </div>

        <div className="sheet__body scroll">
          {/* 音质 */}
          <div className="group">
            <div className="group__head">
              <IconSpark size={13} />
              <span className="group__name">音质</span>
            </div>
            <div className="field">
              <div className="field__col">
                <span className="field__label">无损优先</span>
                <span className="field__hint">
                  {loggedIn
                    ? "优先请求 FLAC 无损流；仅部分投稿提供，失败时自动回落到 192K。"
                    : "需要大会员账号。当前未登录，只会请求 192K AAC-LC。"}
                </span>
              </div>
              <Switch checked={preferFlac} onChange={setPreferFlac} />
            </div>
            <div className="field">
              <span className="field__label">当前输出</span>
              <span className={`badge badge--${quality ? quality.tone : "dim"}`}>
                {quality ? quality.label : "未在播放"}
              </span>
            </div>
          </div>

          {/* 歌词 */}
          <div className="group">
            <div className="group__head">
              <IconLyrics size={13} />
              <span className="group__name">歌词</span>
            </div>
            <div className="field">
              <div className="field__col">
                <span className="field__label">优先显示翻译</span>
                <span className="field__hint">有翻译时在原句下方以蓝色小字显示。</span>
              </div>
              <Switch checked={showTr} onChange={setShowTr} />
            </div>
            <div className="field" style={{ display: "block" }}>
              <span className="field__label" style={{ display: "block", marginBottom: 9 }}>来源优先级</span>
              <div className="orderlist">
                <div className="orderrow">
                  <span className="orderrow__n">01</span>
                  <span>B 站 CC 字幕</span>
                  <span style={{ marginLeft: "auto", fontSize: 10.5, color: "var(--fg-4)" }}>最准</span>
                </div>
                <div className="orderrow">
                  <span className="orderrow__n">02</span>
                  <span>网易云音乐</span>
                  <span style={{ marginLeft: "auto", fontSize: 10.5, color: "var(--fg-4)" }}>含翻译</span>
                </div>
                <div className="orderrow orderrow--muted">
                  <span className="orderrow__n">03</span>
                  <span>无歌词（占位提示）</span>
                </div>
              </div>
            </div>
          </div>

          {/* 外观 */}
          <div className="group">
            <div className="group__head">
              <IconTarget size={13} />
              <span className="group__name">外观</span>
            </div>
            <div className="field">
              <span className="field__label">强调色</span>
              <div className="swatches">
                {ACCENTS.map((a) => (
                  <button
                    key={a.id}
                    className={`swatch ${accent === a.id ? "swatch--on" : ""}`}
                    style={{ background: `linear-gradient(150deg, ${a.soft}, ${a.accent})`, color: a.accent }}
                    onClick={() => setAccent(a.id)}
                    title={a.name}
                    aria-label={a.name}
                  ></button>
                ))}
              </div>
            </div>
            <div className="field">
              <span className="field__label">歌词字号</span>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <span className="mono" style={{ fontSize: 11, color: "var(--fg-4)" }}>{lyricSize}px</span>
                <input
                  className="range"
                  type="range"
                  min="14"
                  max="20"
                  step="0.5"
                  value={lyricSize}
                  onChange={(e) => setLyricSize(parseFloat(e.target.value))}
                  aria-label="歌词字号"
                />
              </div>
            </div>
            <div className="field">
              <span className="field__label">列表密度</span>
              <div className="seg-sm">
                <button className={density === "comfortable" ? "on" : ""} onClick={() => setDensity("comfortable")}>舒适</button>
                <button className={density === "compact" ? "on" : ""} onClick={() => setDensity("compact")}>紧凑</button>
              </div>
            </div>
          </div>

          {/* 中文字体 */}
          <div className="group">
            <div className="group__head">
              <IconTranslate size={13} />
              <span className="group__name">中文字体</span>
            </div>
            <div className="field" style={{ display: "block" }}>
              <div className="seg-sm" style={{ marginBottom: 12 }}>
                {CJK_FONTS.map((f) => (
                  <button
                    key={f.id}
                    className={cjkFont === f.id ? "on" : ""}
                    onClick={() => setCjkFont(f.id)}
                  >
                    {f.name}
                  </button>
                ))}
              </div>
              <span className="field__hint" style={{ display: "block", marginBottom: 9 }}>
                自动探测失败时界面中文会显示为方块，此时在下面填入字体绝对路径。
              </span>
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  className="input"
                  value={cjkPath}
                  onChange={(e) => setCjkPath(e.target.value)}
                  placeholder="/System/Library/Fonts/PingFang.ttc"
                  aria-label="中文字体路径"
                />
                <button className="iconbtn" onClick={onCopyPath} title="复制路径" style={{ width: 32, height: 32, boxShadow: "0 0 0 1px var(--line) inset" }}>
                  <IconCopy size={14} />
                </button>
              </div>
            </div>
          </div>

          {/* 配置文件 */}
          <div className="group">
            <div className="group__head">
              <IconFolder size={13} />
              <span className="group__name">配置文件</span>
            </div>
            <div className="field" style={{ display: "block" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span className="mono" style={{ fontSize: 11, color: "var(--fg-3)", flex: 1, wordBreak: "break-all" }}>
                  ~/Library/Application Support/listenBli/config.json
                </span>
                <button className="iconbtn" onClick={onCopyPath} title="复制配置路径" style={{ width: 30, height: 30 }}>
                  <IconCopy size={13} />
                </button>
              </div>
              <span className="field__hint" style={{ display: "block", marginTop: 8 }}>
                音量、无损开关、歌词偏好与登录凭据都存在这里；Windows 下位于 %APPDATA%\listenBli\config\。
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { LoginModal, SettingsSheet, ACCENTS, CJK_FONTS });

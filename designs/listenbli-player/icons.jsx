// icons.jsx — stroke icon set (Lucide-flavoured: 24px grid, 1.6 stroke).
// Icons are protocol glyphs, not illustration; they keep the UI legible at
// small sizes where text labels would crowd the layout.

const Icon = ({ children, size = 18, className = "", strokeWidth = 1.6, ...rest }) => (
  <svg
    className={`icon ${className}`}
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={strokeWidth}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    {...rest}
  >
    {children}
  </svg>
);

const IconSearch = (p) => (
  <Icon {...p}>
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.2-3.2" />
  </Icon>
);

const IconSettings = (p) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="3.2" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.11A1.7 1.7 0 0 0 8.9 19.3a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.7 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.11A1.7 1.7 0 0 0 4.7 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.7a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.11A1.7 1.7 0 0 0 15 4.7a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.3 9v.03A1.7 1.7 0 0 0 20.86 10H21a2 2 0 1 1 0 4h-.11a1.7 1.7 0 0 0-1.49 1Z" />
  </Icon>
);

const IconPlay = ({ size = 18, ...p }) => (
  <Icon size={size} {...p} strokeWidth={0}>
    <path d="M8 5.6c0-.9 1-1.5 1.8-1L18.4 9.9c.8.5.8 1.7 0 2.2L9.8 17.4c-.8.5-1.8-.1-1.8-1Z" fill="currentColor" />
  </Icon>
);

const IconPause = ({ size = 18, ...p }) => (
  <Icon size={size} {...p} strokeWidth={0}>
    <rect x="7" y="5" width="3.4" height="14" rx="1.4" fill="currentColor" />
    <rect x="13.6" y="5" width="3.4" height="14" rx="1.4" fill="currentColor" />
  </Icon>
);

const IconPrev = (p) => (
  <Icon {...p}>
    <path d="M18 6.5v11L9.5 12Z" fill="currentColor" strokeWidth="0" />
    <path d="M6.6 5.4v13.2" />
  </Icon>
);

const IconNext = (p) => (
  <Icon {...p}>
    <path d="M6 6.5v11L14.5 12Z" fill="currentColor" strokeWidth="0" />
    <path d="M17.4 5.4v13.2" />
  </Icon>
);

const IconVolume = ({ muted = false, ...p }) => (
  <Icon {...p}>
    <path d="M11 5.2 6.6 8.8H3.9v6.4h2.7L11 18.8Z" />
    {muted ? (
      <path d="m15.5 9.5 5 5m0-5-5 5" />
    ) : (
      <>
        <path d="M15 9.2a4 4 0 0 1 0 5.6" />
        <path d="M17.8 6.6a7.6 7.6 0 0 1 0 10.8" />
      </>
    )}
  </Icon>
);

const IconQueue = (p) => (
  <Icon {...p}>
    <path d="M4 7h10M4 12h10M4 17h6" />
    <path d="M17.5 14.2v5.2" />
    <circle cx="17.5" cy="13" r="1.5" />
  </Icon>
);

const IconLyrics = (p) => (
  <Icon {...p}>
    <path d="M5.5 4.6h13a1.4 1.4 0 0 1 1.4 1.4v10a1.4 1.4 0 0 1-1.4 1.4h-6.9L7.2 21v-3.6H5.5A1.4 1.4 0 0 1 4.1 16V6a1.4 1.4 0 0 1 1.4-1.4Z" />
    <path d="M8.4 9.2h7.2M8.4 12.6h4.4" />
  </Icon>
);

const IconLock = (p) => (
  <Icon {...p}>
    <rect x="4.8" y="10.4" width="14.4" height="9.6" rx="2" />
    <path d="M8.4 10.4V7.8a3.6 3.6 0 0 1 7.2 0v2.6" />
    <path d="M12 14.4v1.8" />
  </Icon>
);

const IconRefresh = (p) => (
  <Icon {...p}>
    <path d="M20 11.5a8 8 0 1 0-2.3 6.1" />
    <path d="M20 5.6v5.9h-5.9" />
  </Icon>
);

const IconCheck = (p) => (
  <Icon {...p}>
    <path d="m5.4 12.6 4.3 4.3 8.9-9.9" />
  </Icon>
);

const IconClose = (p) => (
  <Icon {...p}>
    <path d="m6.4 6.4 11.2 11.2M17.6 6.4 6.4 17.6" />
  </Icon>
);

const IconChevron = (p) => (
  <Icon {...p}>
    <path d="m6.6 9.4 5.4 5.2 5.4-5.2" />
  </Icon>
);

const IconFolder = (p) => (
  <Icon {...p}>
    <path d="M4 7.6a1.8 1.8 0 0 1 1.8-1.8h3.1l1.8 2.2h7.5A1.8 1.8 0 0 1 20 9.8v7.6a1.8 1.8 0 0 1-1.8 1.8H5.8A1.8 1.8 0 0 1 4 17.4Z" />
  </Icon>
);

const IconClock = (p) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8" />
    <path d="M12 7.6V12l3.2 2" />
  </Icon>
);

const IconQr = (p) => (
  <Icon {...p}>
    <rect x="4.2" y="4.2" width="6" height="6" rx="1.2" />
    <rect x="13.8" y="4.2" width="6" height="6" rx="1.2" />
    <rect x="4.2" y="13.8" width="6" height="6" rx="1.2" />
    <path d="M14 14h2.2v2.2H14zM17.8 17.8H20V20h-2.2zM13.8 18.4v1.6M18.6 13.8h1.4" />
  </Icon>
);

const IconTranslate = (p) => (
  <Icon {...p}>
    <path d="M4.4 6.6h7.4M8.1 5.2v1.4c0 3.4-1.7 6-4 7.4" />
    <path d="M6.3 11.4c1.2 1.8 2.8 3 4.6 3.6" />
    <path d="m12.6 19.4 3.4-8.6 3.4 8.6M13.8 16.6h4.4" />
  </Icon>
);

const IconSpark = (p) => (
  <Icon {...p}>
    <path d="M12 3.6 13.9 9l5.5 1.9-5.5 1.9L12 18.4 10.1 12.8 4.6 10.9 10.1 9Z" />
  </Icon>
);

const IconCopy = (p) => (
  <Icon {...p}>
    <rect x="9" y="9" width="10.4" height="10.4" rx="2" />
    <path d="M14.6 6.2A1.8 1.8 0 0 0 12.9 4.6H6.4a1.8 1.8 0 0 0-1.8 1.8v6.5c0 .84.58 1.55 1.38 1.73" />
  </Icon>
);

const IconExternal = (p) => (
  <Icon {...p}>
    <path d="M14.4 4.4h5.2v5.2" />
    <path d="M10.6 13.4 19.2 4.8" />
    <path d="M17.2 13.2v4.8a1.6 1.6 0 0 1-1.6 1.6H6a1.6 1.6 0 0 1-1.6-1.6V8.4A1.6 1.6 0 0 1 6 6.8h4.8" />
  </Icon>
);

const IconDownload = (p) => (
  <Icon {...p}>
    <path d="M12 3.6v11.2" />
    <path d="m7.4 10.2 4.6 4.6 4.6-4.6" />
    <path d="M20 14.8v4a1.6 1.6 0 0 1-1.6 1.6H5.6A1.6 1.6 0 0 1 4 18.8v-4" />
  </Icon>
);

const IconMore = (p) => (
  <Icon {...p} strokeWidth={2.4}>
    <path d="M6.4 12h.01M12 12h.01M17.6 12h.01" />
  </Icon>
);

const IconTarget = (p) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="7.6" />
    <circle cx="12" cy="12" r="2.4" />
    <path d="M12 4.4V2.8M12 21.2v-1.6M4.4 12H2.8M21.2 12h-1.6" />
  </Icon>
);

const IconCloseSmall = IconClose;

Object.assign(window, {
  Icon,
  IconSearch,
  IconSettings,
  IconPlay,
  IconPause,
  IconPrev,
  IconNext,
  IconVolume,
  IconQueue,
  IconLyrics,
  IconLock,
  IconRefresh,
  IconCheck,
  IconClose,
  IconCloseSmall,
  IconChevron,
  IconFolder,
  IconClock,
  IconQr,
  IconTranslate,
  IconSpark,
  IconCopy,
  IconExternal,
  IconDownload,
  IconMore,
  IconTarget,
});

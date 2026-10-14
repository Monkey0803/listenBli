// aggregator-icons.jsx — the icon set shared by all three directions.
//
// 16×16 grid, 1.5 stroke, round caps: the same geometry reads as engraved
// instrument labels in direction A, as printed marks in B, and as thin filaments
// in C. Directions vary size/weight/colour, never the shapes.

const AGG_ICONS = {
  search: (w) => (
    <>
      <circle cx="7.1" cy="7.1" r="4.1" strokeWidth={w} />
      <path d="M10.2 10.2 13.6 13.6" strokeWidth={w} />
    </>
  ),
  play: () => <path d="M5.4 3.6 12.4 8l-7 4.4Z" fill="currentColor" stroke="none" />,
  pause: () => (
    <>
      <rect x="5.2" y="3.6" width="2.1" height="8.8" rx="0.6" fill="currentColor" stroke="none" />
      <rect x="8.7" y="3.6" width="2.1" height="8.8" rx="0.6" fill="currentColor" stroke="none" />
    </>
  ),
  prev: () => (
    <>
      <path d="M12 3.8 6.2 8 12 12.2Z" fill="currentColor" stroke="none" />
      <rect x="3.4" y="3.8" width="1.7" height="8.4" rx="0.7" fill="currentColor" stroke="none" />
    </>
  ),
  next: () => (
    <>
      <path d="M4 3.8 9.8 8 4 12.2Z" fill="currentColor" stroke="none" />
      <rect x="10.9" y="3.8" width="1.7" height="8.4" rx="0.7" fill="currentColor" stroke="none" />
    </>
  ),
  volume: (w) => (
    <>
      <path d="M3 6.2h2.2L8.2 3.4v9.2L5.2 9.8H3Z" strokeWidth={w} />
      <path d="M10.6 6a3 3 0 0 1 0 4" strokeWidth={w} />
    </>
  ),
  queue: (w) => (
    <>
      <path d="M3 4.4h10M3 8h10M3 11.6h6.4" strokeWidth={w} />
    </>
  ),
  lyrics: (w) => (
    <>
      <path d="M4 3.4h8v9.2H4Z" strokeWidth={w} />
      <path d="M6 6.4h4M6 9.2h2.6" strokeWidth={w} />
    </>
  ),
  settings: (w) => (
    <>
      <circle cx="8" cy="8" r="2.1" strokeWidth={w} />
      <path d="M8 2.4v1.5M8 12.1v1.5M13.6 8h-1.5M3.9 8H2.4M11.96 4.04l-1.06 1.06M5.1 10.9l-1.06 1.06M11.96 11.96l-1.06-1.06M5.1 5.1 4.04 4.04" strokeWidth={w} />
    </>
  ),
  download: (w) => (
    <>
      <path d="M8 2.8v6.4M5.4 6.8 8 9.4l2.6-2.6" strokeWidth={w} />
      <path d="M3.4 12.4h9.2" strokeWidth={w} />
    </>
  ),
  check: (w) => <path d="M3.6 8.4 6.4 11.2 12.4 5" strokeWidth={w} />,
  plus: (w) => <path d="M8 3.4v9.2M3.4 8h9.2" strokeWidth={w} />,
  more: () => (
    <>
      <circle cx="3.6" cy="8" r="1.05" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r="1.05" fill="currentColor" stroke="none" />
      <circle cx="12.4" cy="8" r="1.05" fill="currentColor" stroke="none" />
    </>
  ),
  close: (w) => <path d="M4.2 4.2 11.8 11.8M11.8 4.2 4.2 11.8" strokeWidth={w} />,
  chevronDown: (w) => <path d="M4.4 6.4 8 10l3.6-3.6" strokeWidth={w} />,
  chevronRight: (w) => <path d="M6.4 4.4 10 8l-3.6 3.6" strokeWidth={w} />,
  link: (w) => (
    <>
      <path d="M6.6 9.4 9.4 6.6" strokeWidth={w} />
      <path d="M7.6 4.4 9 3a2.4 2.4 0 0 1 3.4 3.4L11 7.8" strokeWidth={w} />
      <path d="M8.4 11.6 7 13a2.4 2.4 0 0 1-3.4-3.4L5 8.2" strokeWidth={w} />
    </>
  ),
  plug: (w) => (
    <>
      <path d="M6 2.8v3M10 2.8v3" strokeWidth={w} />
      <path d="M4.4 5.8h7.2v2.1a3.6 3.6 0 0 1-3.6 3.6 3.6 3.6 0 0 1-3.6-3.6Z" strokeWidth={w} />
      <path d="M8 11.5v1.8" strokeWidth={w} />
    </>
  ),
  clock: (w) => (
    <>
      <circle cx="8" cy="8" r="5.4" strokeWidth={w} />
      <path d="M8 4.8V8l2.2 1.4" strokeWidth={w} />
    </>
  ),
  folder: (w) => (
    <>
      <path d="M2.6 5.2h4l1.2-1.6h5.6v8.4H2.6Z" strokeWidth={w} />
    </>
  ),
  disk: (w) => (
    <>
      <circle cx="8" cy="8" r="5.4" strokeWidth={w} />
      <circle cx="8" cy="8" r="1.5" strokeWidth={w} />
    </>
  ),
  users: (w) => (
    <>
      <circle cx="6.2" cy="6" r="2.2" strokeWidth={w} />
      <path d="M2.6 12.6a3.6 3.6 0 0 1 7.2 0" strokeWidth={w} />
      <path d="M10.4 5.1a2.1 2.1 0 0 1 0 4M11.6 12.6a3.4 3.4 0 0 0-1.4-2.8" strokeWidth={w} />
    </>
  ),
  // two streams merging into one — the dedupe mark, used by every direction
  merge: (w) => (
    <>
      <path d="M3 3.6h3.2a2.4 2.4 0 0 1 2.4 2.4v4" strokeWidth={w} />
      <path d="M13 6.4h-1.2a2.4 2.4 0 0 0-2.4 2.4v1.2" strokeWidth={w} />
      <path d="M6 12.4h4" strokeWidth={w} />
    </>
  ),
  filter: (w) => <path d="M2.8 4h10.4L9.4 8.6v4l-2.8-1.6V8.6Z" strokeWidth={w} />,
  wave: () => (
    <>
      <rect x="2.4" y="7.4" width="1.6" height="4.2" rx="0.8" fill="currentColor" stroke="none" />
      <rect x="5.6" y="4.6" width="1.6" height="7" rx="0.8" fill="currentColor" stroke="none" />
      <rect x="8.8" y="6.2" width="1.6" height="5.4" rx="0.8" fill="currentColor" stroke="none" />
      <rect x="12" y="3.4" width="1.6" height="8.2" rx="0.8" fill="currentColor" stroke="none" />
    </>
  ),
  lock: (w) => (
    <>
      <rect x="3.6" y="7" width="8.8" height="6.2" rx="1.4" strokeWidth={w} />
      <path d="M5.8 7V5.6a2.2 2.2 0 0 1 4.4 0V7" strokeWidth={w} />
    </>
  ),
};

/**
 * <Icon name="play" size={14} weight={1.5} />
 *
 * `weight` is the stroke width on the 16-unit grid; a direction that wants
 * heavier engraved marks passes a larger number rather than using a different set.
 */
function Icon({ name, size = 16, weight = 1.5, className, style }) {
  const draw = AGG_ICONS[name];
  if (!draw) return null;
  return (
    <svg
      className={className}
      style={style}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {draw(weight)}
    </svg>
  );
}

Object.assign(window, { Icon, AGG_ICONS });

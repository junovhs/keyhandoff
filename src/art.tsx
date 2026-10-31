// Art for the Calm look: chunky line art for screen subjects (Safe Copy's
// picture, panel glyphs) and thin line icons for small UI.

import { useRef, type PointerEvent } from "react";

const FRONT = "M24 20H49L72 43V88H24Z";
const FRONT_LINES = "M49 20V43H72M35 54H61M35 66H61";
const CHECK = "M55 80L65 90L94 61";
const LOCK_SHACKLE = "M70 70V63a8 8 0 0 1 16 0V70";

/**
 * Safe Copy's picture, the same in every Safe Copy state so nothing moves:
 * two chunky pages that float gently, and a mark in the corner. `check` draws
 * a green check on when the key is caught; `lock` swaps it for a green lock
 * once sealed; `missed` crossfades the pages to a stopwatch in the same place
 * when time runs out. Marks sit on a ground-coloured outline that cuts a gap
 * out of the page behind them.
 */
export function Pages({ mark }: { mark: "none" | "check" | "lock" | "missed" }) {
  return (
    <svg className={`pages ${mark}`} viewBox="0 -2 100 100" width="160" height="160" aria-hidden>
      <rect className="back" x="9" y="3" width="40" height="62" />
      <g className="front">
        <path className="cut" d={FRONT} />
        <path className="page" d={FRONT} />
        <path className="line" d={FRONT_LINES} />
        <path className="check-cut" d={CHECK} pathLength={1} />
        <path className="check" d={CHECK} pathLength={1} />
        <g className="lock">
          <rect className="lock-cut" x="60" y="56" width="36" height="40" rx="4" />
          <path className="lock-shackle" d={LOCK_SHACKLE} />
          <rect className="lock-body" x="64" y="70" width="28" height="22" rx="2.5" />
        </g>
      </g>
      <g className="clock">
        <circle className="clock-face" cx="50" cy="56" r="32" />
        <path className="clock-face" d="M43 13H57M50 13V24M74 27L79 22" />
        <path className="clock-hand" d="M50 56V36" />
        <circle className="clock-pin" cx="50" cy="56" r="4.5" />
      </g>
    </svg>
  );
}

const GLYPH_MARKS = {
  check: { d: CHECK, tone: "ok" },
  send: { d: "M60 78H90M79 67L90 78L79 89", tone: "accent" },
  key: { d: "M57 79a7 7 0 1 0 14 0a7 7 0 1 0 -14 0M71 79H93M87 79V87", tone: "accent" },
} as const;

/**
 * The subject of a panel, in the same chunky line style as Safe Copy's picture:
 * a page (or a program window) with a coloured mark in its corner.
 */
export function Glyph({ mark, base = "page", size = 64 }: { mark: keyof typeof GLYPH_MARKS; base?: "page" | "window"; size?: number }) {
  const { d, tone } = GLYPH_MARKS[mark];
  return (
    <svg className="glyph" viewBox="18 14 84 84" width={size} height={size} aria-hidden>
      {base === "page" ? (
        <>
          <path className="page" d={FRONT} />
          <path className="line" d={FRONT_LINES} />
        </>
      ) : (
        <>
          <rect className="page" x="22" y="24" width="62" height="54" />
          <path className="line" d="M34 42L43 50L34 58M49 60H62" />
        </>
      )}
      <path className="mark-cut" d={d} />
      <path className={`mark ${tone}`} d={d} />
    </svg>
  );
}

/** The KeyHandoff logo, Monolith, coloured by the surrounding text colour. */
export function Logo({ height = 22 }: { height?: number }) {
  return (
    <svg className="logo" viewBox="28 8 64 104" height={height} width={(height * 64) / 104} aria-hidden>
      <path fillRule="evenodd" fill="currentColor" d="M28 8H92V60H72V78H90V90H72V98H90V112H48V60H28ZM50 24V44H70V24Z" />
    </svg>
  );
}

const LINES = {
  close: "M4 4l8 8M12 4l-8 8",
  minimize: "M4 8h8",
  check: "M3.5 8.5l3 3 6-7",
  key: "M2.5 8a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0M7.5 8h6M11.5 8v2.5M13.5 8v2",
  shield: "M8 1.8l5 2v4c0 3.2-2.2 5.2-5 6.4-2.8-1.2-5-3.2-5-6.4v-4zM5.8 8.2l1.6 1.6 3-3.4",
  file: "M4 1.8h5l3 3V14.2H4zM9 1.8V5h3",
  search: "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM10.8 10.8L14 14",
  once: "M3.2 8a4.8 4.8 0 1 0 1.4-3.4M3.2 2.6v3h3",
  alert: "M8 2.2L14.3 13.3H1.7ZM8 6.4v3.2M8 11.3v.1",
  calendar: "M2.5 3.5h11v10h-11zM2.5 6.8h11M5.5 2v3M10.5 2v3M5.5 9.5h1M9.5 9.5h1M5.5 11.5h1",
  forever: "M8 8C6.6 6.1 5.6 5.2 4.3 5.2a2.8 2.8 0 0 0 0 5.6C5.6 10.8 6.6 9.9 8 8s2.4-2.8 3.7-2.8a2.8 2.8 0 0 1 0 5.6C10.4 10.8 9.4 9.9 8 8z",
} as const;

/** A 16px line icon, coloured by the surrounding text colour. */
export function Icon({ name }: { name: keyof typeof LINES }) {
  return (
    <svg className="icon" viewBox="0 0 16 16" aria-hidden>
      <path d={LINES[name]} />
    </svg>
  );
}

/**
 * Pointer handlers that start a native file drag (Rust/GTK) once the pointer
 * has moved a few pixels with the button held, so a plain click still clicks.
 */
export function useDragOut(start: () => void) {
  const origin = useRef<{ x: number; y: number } | null>(null);
  return {
    onPointerDown: (e: PointerEvent) => {
      if (e.button === 0) origin.current = { x: e.clientX, y: e.clientY };
    },
    onPointerMove: (e: PointerEvent) => {
      const o = origin.current;
      if (o && Math.hypot(e.clientX - o.x, e.clientY - o.y) > 4) {
        origin.current = null;
        start();
      }
    },
    onPointerUp: () => {
      origin.current = null;
    },
  };
}

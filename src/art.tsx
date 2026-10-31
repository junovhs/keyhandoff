// Art for the Calm look. Pixel art appears in icon tiles and the Safe Copy
// picture, at 3x or more and whole-number scales so edges stay crisp. Small UI
// icons are line icons: tiny pixel icons read as grainy.

import { useRef, type PointerEvent } from "react";

const ICONS = {
  key: [12, 8, "M1 2h3v1h-3zM0 3h1v1h-1zM4 3h1v1h-1zM0 4h1v1h-1zM2 4h1v1h-1zM4 4h7v1h-7zM0 5h1v1h-1zM4 5h1v1h-1zM8 5h1v1h-1zM10 5h1v1h-1zM1 6h3v1h-3zM8 6h1v1h-1zM10 6h1v1h-1z"],
  shield: [12, 11, "M1 0h10v1h-10zM0 1h12v2h-12zM0 3h5v1h-5zM7 3h5v1h-5zM0 4h4v2h-4zM8 4h4v2h-4zM0 6h5v1h-5zM7 6h5v1h-5zM1 7h10v1h-10zM2 8h8v1h-8zM3 9h6v1h-6zM5 10h2v1h-2z"],
  sealed: [12, 10, "M1 0h10v1h-10zM1 1h1v8h-1zM10 1h1v8h-1zM3 2h6v1h-6zM3 4h4v1h-4zM5 6h3v1h-3zM4 7h5v1h-5zM5 8h3v1h-3zM1 9h10v1h-10z"],
  clip: [10, 10, "M3 0h4v1h-4zM1 1h2v1h-2zM4 1h2v1h-2zM7 1h2v1h-2zM1 2h1v7h-1zM8 2h1v7h-1zM3 3h4v1h-4zM3 5h3v1h-3zM3 7h4v1h-4zM1 9h8v1h-8z"],
  send: [10, 7, "M5 0h1v1h-1zM5 1h2v1h-2zM1 2h8v1h-8zM1 3h9v1h-9zM1 4h8v1h-8zM5 5h2v1h-2zM5 6h1v1h-1z"],
  check: [11, 7, "M9 0h2v1h-2zM8 1h2v1h-2zM7 2h2v1h-2zM0 3h2v1h-2zM6 3h2v1h-2zM1 4h2v1h-2zM5 4h2v1h-2zM2 5h4v1h-4zM3 6h2v1h-2z"],
  file: [10, 9, "M1 0h6v1h-6zM1 1h1v7h-1zM6 1h2v1h-2zM6 2h1v1h-1zM8 2h1v1h-1zM6 3h4v1h-4zM3 4h2v2h-2zM9 4h1v4h-1zM6 5h2v2h-2zM1 8h9v1h-9z"],
  clock: [10, 9, "M3 0h4v1h-4zM1 1h2v1h-2zM7 1h2v1h-2zM1 2h1v1h-1zM4 2h1v2h-1zM8 2h1v1h-1zM0 3h1v3h-1zM9 3h1v3h-1zM4 4h3v1h-3zM1 6h1v1h-1zM8 6h1v1h-1zM1 7h2v1h-2zM7 7h2v1h-2zM3 8h4v1h-4z"],
  close: [7, 7, "M0 0h1v1h-1zM6 0h1v1h-1zM1 1h1v1h-1zM5 1h1v1h-1zM2 2h1v1h-1zM4 2h1v1h-1zM3 3h1v1h-1zM2 4h1v1h-1zM4 4h1v1h-1zM1 5h1v1h-1zM5 5h1v1h-1zM0 6h1v1h-1zM6 6h1v1h-1z"],
  minimize: [7, 7, "M0 3h7v1h-7z"],
} as const;

export type IconName = keyof typeof ICONS;

/** A pixel icon at a whole-number scale, coloured by the surrounding text colour. */
export function Px({ name, scale = 2 }: { name: IconName; scale?: number }) {
  const [w, h, d] = ICONS[name];
  return (
    <svg className="px" viewBox={`0 0 ${w} ${h}`} width={w * scale} height={h * scale} shapeRendering="crispEdges" aria-hidden>
      <path d={d} fill="currentColor" />
    </svg>
  );
}

/** The subject of a screen: a black pixel icon on a white tile. */
export function Tile({ name, scale = 3, size = "md" }: { name: IconName; scale?: number; size?: "sm" | "md" | "lg" }) {
  return (
    <span className={`tile-ico ${size}`}>
      <Px name={name} scale={scale} />
    </span>
  );
}

/**
 * Safe Copy's "copy your key" picture: two pixel pages that take turns bobbing
 * one pixel, in two frames like a sprite. The front page is backed by the
 * ground colour, which also cuts a one-pixel gap out of the page behind it.
 */
export function CopyPages({ scale = 10 }: { scale?: number }) {
  return (
    <svg className="copy-pages" viewBox="0 -1 12 17" width={12 * scale} height={17 * scale} shapeRendering="crispEdges" aria-hidden>
      <path className="back" fill="currentColor" d="M0 0h8v1h-8zM0 1h1v11h-1zM7 1h1v11h-1zM1 11h6v1h-6z" />
      <g className="front">
        <rect x="2" y="2" width="10" height="14" style={{ fill: "var(--ground)" }} />
        <path
          fill="currentColor"
          d="M3 3h6v1h-6zM3 4h1v11h-1zM8 4h1v2h-1zM9 4h1v1h-1zM10 5h1v1h-1zM8 6h3v1h-3zM11 6h1v9h-1zM3 15h9v1h-9zM5 8h5v1h-5zM5 10h5v1h-5zM5 12h3v1h-3z"
        />
      </g>
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

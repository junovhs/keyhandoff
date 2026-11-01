// The paths behind the app's art (art.tsx), in plain TypeScript so the website's
// mock of the app (site/src/components/AppArt.astro) draws exactly the same shapes.

export const FRONT = "M24 20H49L72 43V88H24Z";
export const FRONT_LINES = "M49 20V43H72M35 54H61M35 66H61";
export const CHECK = "M55 80L65 90L94 61";
export const LOCK_SHACKLE = "M70 70V63a8 8 0 0 1 16 0V70";

export const GLYPH_MARKS = {
  check: { d: CHECK, tone: "ok" },
  send: { d: "M60 78H90M79 67L90 78L79 89", tone: "accent" },
  key: { d: "M57 79a7 7 0 1 0 14 0a7 7 0 1 0 -14 0M71 79H93M87 79V87", tone: "accent" },
  stop: { d: "M65 69L85 89M85 69L65 89", tone: "accent" },
} as const;

export const LINES = {
  close: "M4 4l8 8M12 4l-8 8",
  minimize: "M4 8h8",
  lock: "M3.5 7.5h9v6.5h-9zM5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5",
  check: "M3.5 8.5l3 3 6-7",
  key: "M2.5 8a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0M7.5 8h6M11.5 8v2.5M13.5 8v2",
  shield: "M8 1.8l5 2v4c0 3.2-2.2 5.2-5 6.4-2.8-1.2-5-3.2-5-6.4v-4zM5.8 8.2l1.6 1.6 3-3.4",
  file: "M4 1.8h5l3 3V14.2H4zM9 1.8V5h3",
  search: "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM10.8 10.8L14 14",
  once: "M3.2 8a4.8 4.8 0 1 0 1.4-3.4M3.2 2.6v3h3",
  users: "M6 7.6a2.6 2.6 0 1 0 0-5.2 2.6 2.6 0 0 0 0 5.2zM1.4 13.6a4.6 4.6 0 0 1 9.2 0M10.6 2.6a2.6 2.6 0 0 1 0 5M12 9.2a4.6 4.6 0 0 1 2.6 4.4",
  user: "M8 8.2a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM2.6 14.2a5.4 5.4 0 0 1 10.8 0",
  alert: "M8 2.2L14.3 13.3H1.7ZM8 6.4v3.2M8 11.3v.1",
  calendar: "M2.5 3.5h11v10h-11zM2.5 6.8h11M5.5 2v3M10.5 2v3M5.5 9.5h1M9.5 9.5h1M5.5 11.5h1",
  forever: "M8 8C6.6 6.1 5.6 5.2 4.3 5.2a2.8 2.8 0 0 0 0 5.6C5.6 10.8 6.6 9.9 8 8s2.4-2.8 3.7-2.8a2.8 2.8 0 0 1 0 5.6C10.4 10.8 9.4 9.9 8 8z",
} as const;

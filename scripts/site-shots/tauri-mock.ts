// Stands in for @tauri-apps/api so the real screens render in a plain browser
// for the website's screenshots. All data here is sample data.
import type { KeyView, SafeState, View } from "../../src/ipc";

const NOW = Math.floor(Date.now() / 1000);
const DAY = 24 * 60 * 60;

const KEYS: KeyView[] = [
  { id: "1", label: "Dialpad", from: "Ben", added_at: NOW - DAY, expires_at: null, keep: "keep" },
  { id: "2", label: "Stripe (billing)", from: "Priya", added_at: NOW - 3 * DAY, expires_at: NOW + 27 * DAY, keep: "days30" },
  { id: "3", label: "OpenAI", from: null, added_at: NOW - 6 * DAY, expires_at: null, keep: "keep" },
];

function safe(state: SafeState, forName: string | null = "Ann"): View["safe"] {
  return {
    state,
    for_name: forName,
    for_computer: forName && "7F3A-91C2",
    ends_at: Date.now() + 42000,
    file_name: state === "sealed" ? "Key for Ann.handoff" : null,
    message: null,
  };
}

const HOME: View = {
  name: "Ann",
  protection: "dpapi",
  keep: "keep",
  keys: KEYS,
  pending: 0,
  safe: safe("off", null),
  accepted: null,
  asked: null,
  notice: null,
};

const SCENES: Record<string, Partial<View>> = {
  home: {},
  ask: { keys: [], pending: 1, asked: "Key request from Ann.handoff" },
  listening: { name: "Ben", keys: [], safe: safe("listening") },
  sealed: { name: "Ben", keys: [], safe: safe("sealed") },
  accepted: { accepted: { ...KEYS[0], added_at: NOW } },
};

const scene = new URLSearchParams(location.search).get("s") ?? "home";

export async function invoke(cmd: string): Promise<unknown> {
  return cmd === "view" ? { ...HOME, ...SCENES[scene] } : undefined;
}

export async function listen(): Promise<() => void> {
  return () => {};
}

export function getCurrentWindow() {
  return { minimize: async () => {}, close: async () => {} };
}

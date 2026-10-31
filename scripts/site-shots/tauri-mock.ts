// Stands in for @tauri-apps/api so the real screens render in a plain browser,
// for the website's screenshots and the screen board. All data here is sample data.
import type { KeyView, SafeState, View } from "../../src/ipc";

const NOW = Math.floor(Date.now() / 1000);
const DAY = 24 * 60 * 60;

const KEYS: KeyView[] = [
  { id: "1", label: "Dialpad", from: "Ben", added_at: NOW - DAY, expires_at: null, keep: "keep" },
  { id: "2", label: "Stripe (billing)", from: "Priya", added_at: NOW - 3 * DAY, expires_at: NOW + 27 * DAY, keep: "days30" },
  { id: "3", label: "OpenAI", from: null, added_at: NOW - 6 * DAY, expires_at: null, keep: "keep" },
];

function safe(state: SafeState, forName: string | null = "Ann", apps: string[] = []): View["safe"] {
  return {
    state,
    for_name: forName,
    for_computer: forName && "72ca77a8cc39c3ba",
    ends_at: Date.now() + 42000,
    file_name: state === "sealed" ? "Key for Ann.handoff" : null,
    message: null,
    apps,
  };
}

const HOME: View = {
  name: "Ann",
  computer: "72ca77a8cc39c3ba",
  protection: "dpapi",
  keep: "keep",
  keys: KEYS,
  pending: 0,
  safe: safe("off", null),
  accepted: null,
  asked: null,
  notice: null,
  program: null,
};

const SCENES: Record<string, Partial<View>> = {
  home: {},
  "home-empty": { keys: [] },
  ask: { keys: [], pending: 1, asked: "Key request from Ann.handoff" },
  listening: { name: "Ben", keys: [], safe: safe("listening") },
  "caught-ann": { name: "Ben", keys: [], safe: safe("caught") },
  sealed: { name: "Ben", keys: [], safe: safe("sealed") },
  accepted: { accepted: { ...KEYS[0], added_at: NOW } },
  "listening-you": { safe: safe("listening", null) },
  "caught-you": { safe: safe("caught", null) },
  "home-added": { notice: "OpenAI is in your keys." },
  program: {
    program: { name: "python3", path: "/usr/bin/python3", command: "python3 dialpad_report.py", folder: "~/code/report-bot", keys: ["Dialpad"] },
  },
  "home-copied": { notice: "Copied DIALPAD_API_KEY=handoff:Dialpad" },
  "listening-apps": { safe: safe("listening", null, ["Ditto"]) },
  timeout: { safe: safe("timeout", null) },
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

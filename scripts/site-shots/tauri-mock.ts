// Stands in for @tauri-apps/api so the real screens render in a plain browser,
// for the website's screenshots and the screen board. All data here is sample data.
import type { ContactView, FolderTeamView, GateView, KeyView, SafeState, TeamView, View } from "../../src/ipc";

const NOW = Math.floor(Date.now() / 1000);
const DAY = 24 * 60 * 60;

const KEYS: KeyView[] = [
  { id: "1", label: "Dialpad", from: "Ben", added_at: NOW - DAY, expires_at: null, keep: "keep", team: null },
  { id: "2", label: "Stripe (billing)", from: "Priya", added_at: NOW - 3 * DAY, expires_at: NOW + 27 * DAY, keep: "days30", team: "Platform" },
  { id: "3", label: "OpenAI", from: null, added_at: NOW - 6 * DAY, expires_at: null, keep: "keep", team: null },
];

const ANN: ContactView = { id: "a", name: "Ann", computer: "72ca77a8cc39c3ba", sends_left: 12 };
const CONTACTS: ContactView[] = [ANN, { id: "p", name: "Priya", computer: "e41b09d2a77f3c10", sends_left: 9 }];

const TEAMS: TeamView[] = [{ id: "t", name: "Platform", members: CONTACTS, keys: 2 }];

const FOLDER = "/home/ben/Dropbox/Platform";
const KEPT: FolderTeamView = { id: "f", name: "Platform", folder: FOLDER, keeper: "Ben", keeping: true, members: [], asking: [], waiting: false, people: [], to_replace: [] };

function safe(state: SafeState, forName: string | null = "Ann", apps: string[] = [], read = false): View["safe"] {
  return {
    state,
    for_name: forName,
    for_team: false,
    ends_at: Date.now() + 42000,
    file_name: state === "sealed" ? "Key for Ann.handoff" : null,
    message: null,
    apps,
    read,
    replacing: null,
  };
}

const HOME: View = {
  name: "Ann",
  computer: "72ca77a8cc39c3ba",
  protection: "passphrase",
  remembered: false,
  lock_after: 15,
  pin: false,
  tpm: "ready",
  keep: "keep",
  keys: KEYS,
  pending: 0,
  safe: safe("off", null),
  accepted: null,
  asked: null,
  notice: null,
  problem: null,
  program: null,
  contacts: CONTACTS,
  added: null,
  teams: TEAMS,
  folder_teams: [],
  invited: null,
};

const SCENES: Record<string, Partial<View>> = {
  home: {},
  "home-empty": { keys: [] },
  "home-ben": { name: "Ben" },
  card: { keys: [], asked: "KeyHandoff card - Ann.handoff" },
  added: { name: "Ben", keys: [], added: ANN },
  "send-empty": { name: "Ben", keys: [], contacts: [] },
  "sealed-team": { name: "Ben", safe: { ...safe("sealed", "Platform"), for_team: true, file_name: "Key for Platform.handoff" } },
  "home-team": { notice: "2 Platform keys from Ben are in your keys." },
  listening: { safe: safe("listening", null) },
  sealed: { name: "Ben", keys: [], safe: safe("sealed") },
  "home-low": (() => {
    const priya = { ...CONTACTS[1], sends_left: 2 };
    return { name: "Ben", contacts: [ANN, priya], teams: [{ ...TEAMS[0], members: [ANN, priya] }] };
  })(),
  "sealed-used-up": { name: "Ben", keys: [], safe: { ...safe("sealed"), message: "That was the last key Ann's card could take. Ask Ann for a new one." } },
  accepted: { accepted: { ...KEYS[0], added_at: NOW } },
  invite: { keys: [], invited: { team: "Platform", keeper: "Ben", computer: "9b3e51f07a2c44d1", folder: "/home/ann/Dropbox/Platform" } },
  "invite-elsewhere": { keys: [], invited: { team: "Platform", keeper: "Ben", computer: "9b3e51f07a2c44d1", folder: null } },
  "folder-new": { name: "Ben", contacts: [], teams: [] },
  "folder-asking": { name: "Ben", contacts: [], teams: [], folder_teams: [{ ...KEPT, asking: [ANN, CONTACTS[1]] }] },
  "folder-kept": { name: "Ben", contacts: [], teams: [], folder_teams: [{ ...KEPT, members: ["Ann", "Priya"] }] },
  "folder-share": { name: "Ben", contacts: [], teams: [], keys: KEYS.map((k) => ({ ...k, from: null })), folder_teams: [{ ...KEPT, members: ["Ann", "Priya"] }] },
  "folder-left": {
    name: "Ben",
    contacts: [],
    teams: [],
    folder_teams: [{ ...KEPT, members: ["Ann"], people: [ANN], to_replace: ["Stripe (billing)", "OpenAI"] }],
    notice: "Priya is off Platform and gets nothing new. Replace the 2 keys they had with the provider, then click Replace on each.",
  },
  "home-removed": { notice: "Ben took you off Platform. Keys it gave you stay in your keys but won't be updated." },
  "replacing": { name: "Ben", safe: { ...safe("listening", null), replacing: "Stripe (billing)" } },
  "replace-caught": { name: "Ben", safe: { ...safe("caught", null), replacing: "Stripe (billing)" } },
  "home-replaced": { name: "Ben", notice: "Stripe (billing) is replaced. Programs using it get the new key. Platform gets it through the folder." },
  "home-updated": { notice: "Stripe (billing) was updated by Ben." },
  "folder-waiting": { keys: [], contacts: [], teams: [], folder_teams: [{ ...KEPT, folder: "/home/ann/Dropbox/Platform", keeping: false, waiting: true }] },
  "listening-you": { safe: safe("listening", null) },
  "caught-you": { safe: safe("caught", null) },
  "home-added": { notice: "OpenAI is in your keys." },
  program: {
    program: { name: "python3", path: "/usr/bin/python3", command: "python3 dialpad_report.py", folder: "~/code/report-bot", keys: ["Dialpad"] },
  },
  "home-copied": { notice: "Copied DIALPAD_API_KEY=handoff:Dialpad" },
  "listening-apps": { safe: safe("listening", null, ["Ditto"]) },
  "listening-read-app": { safe: safe("listening", null, ["Ditto"], true) },
  "listening-read": { safe: safe("listening", null, [], true) },
  timeout: { safe: safe("timeout", null) },
};

const OPEN: GateView = { state: "open", migrating: false, recovery: null, min_passphrase: 12, pin: false, min_pin: 4, update: null };

// Before the vault opens: the scenes with no view.
const GATES: Record<string, Partial<GateView>> = {
  setup: { state: "setup" },
  migrate: { state: "setup", migrating: true },
  recovery: { recovery: "7K2Q-9MXD-4RTV-HB3N-0PWE-6YJC-8FAZ-2KGS" },
  locked: { state: "locked" },
  "locked-pin": { state: "locked", pin: true },
  "home-update": { update: { version: "0.1.1", asks_password: false } },
};

const scene = new URLSearchParams(location.search).get("s") ?? "home";

export async function invoke(cmd: string): Promise<unknown> {
  if (cmd === "gate") return { ...OPEN, ...GATES[scene] };
  if (cmd === "suggest") return "flashing marathon rebate handset repeater";
  return cmd === "view" ? { ...HOME, ...SCENES[scene] } : undefined;
}

export async function listen(): Promise<() => void> {
  return () => {};
}

export function getCurrentWindow() {
  return { minimize: async () => {}, close: async () => {} };
}

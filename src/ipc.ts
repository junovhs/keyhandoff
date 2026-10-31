// The only bridge to the Rust core. These calls carry names, states
// and ids; no secret value ever crosses into TypeScript.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Keep = "once" | "days30" | "keep";
export type SafeState = "off" | "listening" | "timeout" | "failed" | "caught" | "sealed";

export interface KeyView {
  id: string;
  label: string;
  /** Who sent it; null when you added it yourself. */
  from: string | null;
  added_at: number;
  expires_at: number | null;
  keep: Keep;
  /** The team it was shared with, if any. */
  team: string | null;
}

export interface SafeView {
  state: SafeState;
  /** The requester; null when catching a key for yourself. */
  for_name: string | null;
  /** The requester's computer fingerprint, to check who is asking. */
  for_computer: string | null;
  /** It's for a team: every member gets their own copy. */
  for_team: boolean;
  /** Unix ms when listening stops. */
  ends_at: number;
  file_name: string | null;
  message: string | null;
  /** Known clipboard apps that were running when Safe Copy armed. */
  apps: string[];
  /** Something read the clipboard bait while Safe Copy listened. */
  read: boolean;
}

/** A program asking for kept keys, waiting for Allow. */
export interface ProgramView {
  /** `python3`, `node.exe`. */
  name: string;
  path: string;
  /** `python3 report.py`; null when the system won't say. */
  command: string | null;
  /** Where it runs; null when the system won't say. */
  folder: string | null;
  keys: string[];
}

export type Answer = "always" | "once" | "deny";

/** Someone whose card you opened. */
export interface ContactView {
  id: string;
  name: string;
  /** Their computer's fingerprint, to check with them. */
  computer: string;
  /** Keys you can still send with the card you have. */
  sends_left: number;
}

/** A team on this computer: a name and some of your contacts. */
export interface TeamView {
  id: string;
  name: string;
  members: ContactView[];
  /** Keys you hold that were shared with this team. */
  keys: number;
}

export interface View {
  name: string;
  /** This computer's fingerprint, which people you ask will see. */
  computer: string;
  protection: string;
  keep: Keep;
  keys: KeyView[];
  pending: number;
  safe: SafeView;
  accepted: KeyView | null;
  asked: string | null;
  notice: string | null;
  /** Something went wrong outside a command, such as opening a file. */
  problem: string | null;
  program: ProgramView | null;
  contacts: ContactView[];
  /** Whose card was just opened. */
  added: ContactView | null;
  teams: TeamView[];
}

export const api = {
  view: () => invoke<View>("view"),
  copyCard: () => invoke<void>("copy_card"),
  sendTo: (id: string) => invoke<void>("send_to", { id }),
  saveTeam: (id: string | null, name: string, members: string[]) => invoke<string>("save_team", { id, name, members }),
  deleteTeam: (id: string) => invoke<void>("delete_team", { id }),
  sendToTeam: (id: string) => invoke<void>("send_to_team", { id }),
  welcome: (team: string, member: string) => invoke<void>("welcome", { team, member }),
  add: () => invoke<void>("add"),
  retry: () => invoke<void>("retry"),
  paste: () => invoke<void>("paste"),
  seal: (label: string) => invoke<void>("seal", { label }),
  keepOwn: (label: string, keep: Keep) => invoke<void>("keep_own", { label, keep }),
  copySealed: () => invoke<void>("copy_sealed"),
  copyRequest: () => invoke<void>("copy_request"),
  dragOut: (what: "sealed" | "request") => invoke<void>("drag_out", { what }),
  closeSafe: () => invoke<void>("close_safe"),
  setKeep: (id: string, keep: Keep) => invoke<void>("set_keep", { id, keep }),
  forget: (id: string) => invoke<void>("forget", { id }),
  copyPointer: (id: string) => invoke<void>("copy_pointer", { id }),
  setName: (name: string) => invoke<void>("set_name", { name }),
  answer: (answer: Answer) => invoke<void>("answer", { answer }),
  dismiss: () => invoke<void>("dismiss"),
  ready: () => invoke<void>("ready"),
};

/** The current view, refreshed whenever the core reports a change. */
export function useView(): View | null {
  const [view, setView] = useState<View | null>(null);
  useEffect(() => {
    const refresh = () => void api.view().then(setView);
    refresh();
    const off = listen("changed", refresh);
    return () => void off.then((f) => f());
  }, []);
  return view;
}

export const KEEP_CHOICES: { keep: Keep; label: string; icon: "once" | "calendar" | "forever" }[] = [
  { keep: "once", label: "Use once", icon: "once" },
  { keep: "days30", label: "Keep 30 days", icon: "calendar" },
  { keep: "keep", label: "Keep forever", icon: "forever" },
];

/** Run a command; resolve to its error text, or null on success. */
export async function attempt(p: Promise<unknown>): Promise<string | null> {
  try {
    await p;
    return null;
  } catch (e) {
    return String(e);
  }
}

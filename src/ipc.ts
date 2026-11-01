// The only bridge to the Rust core. These calls carry names, states
// and ids; no key ever crosses into TypeScript.
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
  /** Who a sealed key is for; null while catching a key of your own. */
  for_name: string | null;
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
  /** The key a new value is being caught for, when replacing one. */
  replacing: string | null;
  /** Sealed for someone whose computer is nearby: it can go straight there. */
  nearby: boolean;
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
  /** The AI coding agent that started it ("Claude Code"); null when none was found. */
  agent: string | null;
}

/** `30min`: offered instead of Always when an AI agent started the program. */
export type Answer = "always" | "30min" | "once" | "deny";

/** Someone whose card you opened. */
export interface ContactView {
  id: string;
  name: string;
  /** Their computer's fingerprint, to check with them. */
  computer: string;
  /** Keys you can still send with the card you have. */
  sends_left: number;
  /** Their computer is on this network with Nearby on. */
  nearby: boolean;
}

/** A team on this computer: a name and some of your contacts. */
export interface TeamView {
  id: string;
  name: string;
  members: ContactView[];
  /** Keys you hold that were shared with this team. */
  keys: number;
}

/** Adding someone nearby. */
export interface PairingView {
  /** When it stops by itself, in Unix milliseconds. */
  ends_at: number;
  /** People nearby who are adding people too; id is their computer. */
  people: ContactView[];
  /** The person you're swapping cards with. */
  with: string | null;
  /** The code to compare with theirs, once both cards are here. */
  code: string | null;
}

/** A team kept in a folder the team already syncs. */
export interface FolderTeamView {
  id: string;
  name: string;
  folder: string;
  keeper: string;
  /** You keep it: you add people and write its keys. */
  keeping: boolean;
  /** Who is on it, as of the last version. */
  members: string[];
  /** For the keeper: people who asked to join and wait for Add. */
  asking: ContactView[];
  /** For a joiner: asked, and the keeper hasn't added you yet. */
  waiting: boolean;
  /** For the keeper: people on the team, to remove. */
  people: ContactView[];
  /** For the keeper: team keys someone who left still has, to replace with the provider. */
  to_replace: string[];
}

/** A team invite you opened, waiting for Join. */
export interface InviteView {
  team: string;
  keeper: string;
  /** The keeper's computer fingerprint, to check with them. */
  computer: string;
  /** The folder it was opened from; null when it came some other way. */
  folder: string | null;
}

export interface View {
  name: string;
  /** This computer's fingerprint, which people you ask will see. */
  computer: string;
  protection: string;
  /** "Remember on this computer" is on: it opens without the passphrase. */
  remembered: boolean;
  /** Minutes without use before it locks; null for never. */
  lock_after: number | null;
  /** A TPM PIN unlocks it. */
  pin: boolean;
  /** This computer's TPM: ready, no-access (not in the tss group) or missing. */
  tpm: "ready" | "no-access" | "missing";
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
  folder_teams: FolderTeamView[];
  invited: InviteView | null;
  /** Nearby is on. */
  nearby: boolean;
  pairing: PairingView | null;
}

/** Before the vault opens: no passphrase yet, locked, or open. */
export interface GateView {
  state: "setup" | "locked" | "open";
  /** An older profile whose keys move behind the new passphrase. */
  migrating: boolean;
  /** The recovery code, shown once after setup to write down. */
  recovery: string | null;
  min_passphrase: number;
  /** A TPM PIN unlocks it, so the lock screen asks for that first. */
  pin: boolean;
  min_pin: number;
  /** A signed update, downloaded and waiting for a restart. */
  update: { version: string; asks_password: boolean } | null;
}

// The passphrase and recovery code are the one thing typed or shown here
// that guards keys: they go straight to Rust and are never stored.
export const gateApi = {
  gate: () => invoke<GateView>("gate"),
  suggest: () => invoke<string>("suggest"),
  setUp: (passphrase: string) => invoke<void>("set_up", { passphrase }),
  unlock: (passphrase: string) => invoke<void>("unlock", { passphrase }),
  unlockPin: (pin: string) => invoke<void>("unlock_pin", { pin }),
  addPin: (pin: string) => invoke<void>("add_pin", { pin }),
  removePin: () => invoke<void>("remove_pin"),
  recover: (code: string, passphrase: string) => invoke<void>("recover", { code, passphrase }),
  recoveryDone: () => invoke<void>("recovery_done"),
  lock: () => invoke<void>("lock"),
  remember: (on: boolean) => invoke<void>("remember", { on }),
  /** Install the waiting update and restart into it. */
  updateNow: () => invoke<void>("update_now"),
  setLockAfter: (minutes: number | null) => invoke<void>("set_lock_after", { minutes }),
};

/** The gate, refreshed whenever the core reports a change. */
export function useGate(): GateView | null {
  const [gate, setGate] = useState<GateView | null>(null);
  useEffect(() => {
    const refresh = () => void gateApi.gate().then(setGate);
    refresh();
    const off = listen("changed", refresh);
    return () => void off.then((f) => f());
  }, []);
  return gate;
}

export const api = {
  view: () => invoke<View>("view"),
  copyCard: () => invoke<void>("copy_card"),
  saveTeam: (id: string | null, name: string, members: string[]) => invoke<string>("save_team", { id, name, members }),
  deleteTeam: (id: string) => invoke<void>("delete_team", { id }),
  /** The system folder picker; null when the person cancels. */
  pickFolder: () => invoke<string | null>("pick_folder"),
  makeFolderTeam: (name: string, folder: string) => invoke<string>("make_folder_team", { name, folder }),
  /** Join the invite just opened; null uses the folder it was opened from. */
  joinTeam: (folder: string | null) => invoke<void>("join_team", { folder }),
  closeInvite: () => invoke<void>("close_invite"),
  addToTeam: (team: string, member: string) => invoke<void>("add_to_team", { team, member }),
  turnAway: (team: string, member: string) => invoke<void>("turn_away", { team, member }),
  /** Take someone off a team folder you keep; what they had waits to be replaced. */
  removeFromTeam: (team: string, member: string) => invoke<void>("remove_from_team", { team, member }),
  /** Share a saved key with a team folder you keep; members get it through the folder. */
  shareWithFolderTeam: (team: string, key: string) => invoke<void>("share_with_folder_team", { team, key }),
  /** Catch a new value for a key you added, keeping its name and allowed programs. */
  replace: (key: string) => invoke<void>("replace", { key }),
  keepReplacement: () => invoke<void>("keep_replacement"),
  /** Let people on this network find this computer, and take keys from people on your list. */
  setNearby: (on: boolean) => invoke<void>("set_nearby", { on }),
  /** Hand the sealed key straight to its person's computer. */
  sendNearby: () => invoke<void>("send_nearby"),
  /** Add someone nearby: both people choose it, swap cards and compare a code. */
  startPairing: () => invoke<void>("start_pairing"),
  cancelPairing: () => invoke<void>("cancel_pairing"),
  pairWith: (computer: string) => invoke<void>("pair_with", { computer }),
  confirmPairing: () => invoke<void>("confirm_pairing"),
  /** Forget a contact and take them off your teams. */
  forgetContact: (id: string) => invoke<void>("forget_contact", { id }),
  /** Seal a key you already hold to a contact, or to every member of a team. */
  sendKept: (contact: string, key: string) => invoke<void>("send_kept", { contact, key }),
  sendKeptToTeam: (team: string, key: string) => invoke<void>("send_kept_to_team", { team, key }),
  welcome: (team: string, member: string) => invoke<void>("welcome", { team, member }),
  add: () => invoke<void>("add"),
  retry: () => invoke<void>("retry"),
  paste: () => invoke<void>("paste"),
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
export function useView(open: boolean): View | null {
  const [view, setView] = useState<View | null>(null);
  useEffect(() => {
    if (!open) return;
    const refresh = () => void api.view().then(setView);
    refresh();
    const off = listen("changed", refresh);
    return () => void off.then((f) => f());
  }, [open]);
  return open ? view : null;
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

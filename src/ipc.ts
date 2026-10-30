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
}

export interface SafeView {
  state: SafeState;
  /** The requester; null when catching a key for yourself. */
  for_name: string | null;
  /** The requester's computer fingerprint, to check who is asking. */
  for_computer: string | null;
  /** Unix ms when listening stops. */
  ends_at: number;
  file_name: string | null;
  message: string | null;
}

export interface View {
  name: string;
  protection: string;
  keep: Keep;
  keys: KeyView[];
  pending: number;
  safe: SafeView;
  accepted: KeyView | null;
  asked: string | null;
  notice: string | null;
}

export const api = {
  view: () => invoke<View>("view"),
  ask: () => invoke<void>("ask"),
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
  setName: (name: string) => invoke<void>("set_name", { name }),
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

export const KEEP_CHOICES: { keep: Keep; label: string }[] = [
  { keep: "once", label: "Once" },
  { keep: "days30", label: "30 days" },
  { keep: "keep", label: "Keep" },
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

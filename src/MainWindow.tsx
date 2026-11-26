import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Glyph, Icon, Logo, MakerMark, VaultMark, useDragOut } from "./art";
import {
  api,
  typingApi,
  attempt,
  gateApi,
  KEEP_CHOICES,
  useGate,
  useView,
  type ContactView,
  type FolderTeamView,
  type PairingView,
  type InviteView,
  type Keep,
  type KeyView,
  type ProgramView,
  type AlertView,
  type TeamView,
  type View,
} from "./ipc";
import { enter, flip, stagger } from "./motion";
import { Gate } from "./Gate";
import { HISTORY } from "./history";
import { Presence } from "./Presence";
import { SafeCopy } from "./SafeCopy";
import { CodeSheet, ImportResult, ImportSheet, ImportStart } from "./Visual";
import { ArrowUpRight, Bot, Check, ChevronDown, Copy, Download, FileInput, Folder, FolderOpen, IdCard, Keyboard, KeyRound, LockKeyhole, Plus, RefreshCw, Send, Server, Settings, SquareTerminal, X } from "lucide-react";

const DAY_MS = 24 * 60 * 60 * 1000;

export function MainWindow() {
  const gate = useGate();
  const open = gate?.state === "open" && !gate.recovery;
  const view = useView(open);
  const stage = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const shown = useRef(false);

  useEffect(() => {
    // The window starts hidden and is shown once React has rendered, so it
    // never flashes white. (No requestAnimationFrame here: webviews don't run
    // it while the window is hidden.)
    if (!gate || shown.current) return;
    shown.current = true;
    stagger(stage.current);
    void api.ready();
  }, [gate]);

  // Ctrl+V anywhere pastes a KeyHandoff file or message (Rust reads the clipboard).
  // Typing anywhere, "/" or Ctrl+F jumps to the key search.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
      const mod = e.ctrlKey || e.metaKey;
      if (!typing && mod && e.key.toLowerCase() === "v" && !document.querySelector(".sheet, .safe, .gate, dialog[open]")) {
        e.preventDefault();
        void run(api.paste());
        return;
      }
      const search = document.getElementById(SEARCH_ID);
      const covered = document.querySelector(".sheet, .safe, .gate, dialog[open]") !== null;
      if (typing || covered || !(search instanceof HTMLInputElement)) return;
      if ((mod && e.key.toLowerCase() === "f") || e.key === "/") {
        e.preventDefault();
        search.focus();
      } else if (!mod && !e.altKey && e.key.length === 1 && e.key !== " ") {
        search.focus(); // the key press itself lands in the box
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const run = async (p: Promise<unknown>) => setError(await attempt(p));
  const win = getCurrentWindow();
  // A program waiting for Allow keeps its buttons clear.
  const bad = error ?? view?.problem ?? null;
  const toast = view?.program ? null : (bad ?? view?.notice ?? null);
  const safeOn = view !== null && view.safe.state !== "off";
  // Send a key: open at the person-or-team list, or straight at someone's keys.
  const [sending, setSending] = useState<To | "pick" | null>(null);
  const [settings, setSettings] = useState(false);
  const [history, setHistory] = useState(false);
  const [security, setSecurity] = useState(false);
  const [code, setCode] = useState(false);
  // The team being edited, "new" for a new one.
  const [editing, setEditing] = useState<TeamView | "new" | null>(null);

  const overlay = Boolean(view && (safeOn || sending || editing || security || code || settings || view.accepted || view.invited ||
    view.pairing || view.asked || view.added || view.import || view.import_receipt || view.alert || view.program));
  useLayoutEffect(() => { if (stage.current) stage.current.inert = overlay; }, [overlay]);

  return (
    <div className="app">
      <header className="titlebar" data-tauri-drag-region>
        <button className="brand" aria-label="Version history" title="Version history" onClick={() => setHistory(true)}><Logo /></button>
        <span className="name" data-tauri-drag-region>KeyHandoff</span>
        {gate?.update && (
          <button
            className="update"
            title={`KeyHandoff ${gate.update.version} is downloaded and verified.${gate.update.asks_password ? " Installing asks for your password." : ""} Restarting locks KeyHandoff.`}
            onClick={() => void run(gateApi.updateNow())}
          >
            Restart to update
          </button>
        )}
        {open && view && !view.remembered && (
          <button aria-label="Lock" title="Lock KeyHandoff" onClick={() => void run(gateApi.lock())}>
            <Icon name="lock" />
          </button>
        )}
        <button aria-label="Minimize" onClick={() => void win.minimize()}>
          <Icon name="minimize" />
        </button>
        <button aria-label="Close" onClick={() => void win.close()}>
          <Icon name="close" />
        </button>
      </header>
      <main className="stage" ref={stage}>
        <Presence show={Boolean(gate && !open)}>{gate && !open && <Gate gate={gate} />}</Presence>
        <Presence show={Boolean(view && !safeOn)}>{view && !safeOn && <Home view={view} run={run} onCard={() => void run(api.prepareCard())} onSend={() => setSending("pick")} onSettings={() => setSettings(true)} onCode={() => setCode(true)} />}</Presence>
      </main>
      <Presence show={history} fx="pop" inMs={320} outMs={220}>{history && <HistoryDialog onClose={() => setHistory(false)} />}</Presence>
      <Presence show={Boolean(view && safeOn)} fx="fade">{view && safeOn && <SafeCopy view={view} />}</Presence>
      <Presence show={Boolean(view?.accepted)}>{view?.accepted && <AcceptSheet item={view.accepted} run={run} />}</Presence>
      <Presence show={Boolean(view?.invited && !view.accepted)}>{view?.invited && !view.accepted && <InviteSheet invite={view.invited} run={run} />}</Presence>
      <Presence show={Boolean(view?.pairing && !view.added)}>{view?.pairing && !view.added && <PairingSheet pairing={view.pairing} run={run} />}</Presence>
      <Presence show={Boolean(view?.asked && !view.accepted)}>{view?.asked && !view.accepted && <CardSheet fileName={view.asked} computer={view.computer} run={run} />}</Presence>
      <Presence show={Boolean(view?.added)}>{view?.added && (
        <ContactSheet contact={view.added} onSend={(c) => void api.dismiss().then(() => setSending({ team: false, id: c.id, name: c.name }))} />
      )}</Presence>
      <Presence show={Boolean(view && sending && !safeOn && !editing && !view.pairing && !view.added)}>{view && sending && !safeOn && !editing && !view.pairing && !view.added && (
        <SendSheet view={view} run={run} start={sending === "pick" ? null : sending} onEdit={setEditing} onClose={() => setSending(null)} />
      )}</Presence>
      <Presence show={Boolean(view && editing && !safeOn)}>{view && editing && !safeOn && (
        <TeamSheet
          team={editing === "new" ? null : (view.teams.find((t) => t.id === editing.id) ?? null)}
          contacts={view.contacts}
          run={run}
          onClose={() => setEditing(null)}
        />
      )}</Presence>
      <Presence show={Boolean(view && settings && !safeOn && !security)}>{view && settings && !safeOn && !security && <SettingsSheet view={view} run={run} onSecurity={() => setSecurity(true)} onClose={() => setSettings(false)} />}</Presence>
      <Presence show={Boolean(view && security && !safeOn)}>{view && security && !safeOn && <SecuritySheet view={view} run={run} onClose={() => setSecurity(false)} />}</Presence>
      <Presence show={Boolean(view && code && !safeOn)}>{view && code && !safeOn && <CodeSheet view={view} onClose={() => setCode(false)} />}</Presence>
      <Presence show={Boolean(view?.import_receipt && !safeOn)}>{view?.import_receipt && !safeOn && <ImportResult receipt={view.import_receipt} run={run} />}</Presence>
      <Presence show={Boolean(view?.import && !safeOn)}>{view?.import && !safeOn && <ImportSheet imp={view.import} keep={view.keep} run={run} />}</Presence>
      <Presence show={Boolean(view?.alert && !view.program)}>{view?.alert && !view.program && <AlertSheet alert={view.alert} />}</Presence>
      <Presence show={Boolean(view?.program)}>{view?.program && <ProgramSheet key={view.program.id} program={view.program} run={run} />}</Presence>
      <Presence show={Boolean(toast)} fx="fade" inMs={220} outMs={220}>{toast && (
        <button className={`toast ${bad ? "bad" : ""}`} onClick={() => { setError(null); void api.dismiss(); }}>
          {!bad && <Icon name="check" />}
          {toast}
        </button>
      )}</Presence>
    </div>
  );
}

type Run = (p: Promise<unknown>) => Promise<void>;

/** Window widths where the vault changes shape: wide puts the heading beside the buttons and keys in tiles. */
const WIDE_AT = 820;
const THREE_COLUMNS_AT = 1120;

/**
 * Pick the vault's shape from the stage's width: narrow, wide with two
 * columns, or wide with three. Only these three, and each change glides.
 */
function useVaultShape(anchor: React.RefObject<HTMLElement>) {
  useLayoutEffect(() => {
    const stage = anchor.current?.closest<HTMLElement>(".stage");
    if (!stage) return;
    let frame = 0;
    const apply = (animate: boolean) => {
      frame = 0;
      const w = stage.clientWidth;
      const shape = w < WIDE_AT ? "narrow" : w < THREE_COLUMNS_AT ? "wide-2" : "wide-3";
      if (stage.dataset.shape === shape) return;
      const set = () => { stage.dataset.shape = shape; };
      if (!animate) return set();
      const ledger = stage.querySelector(".key-ledger")?.getBoundingClientRect();
      const rows = Array.from(stage.querySelectorAll<HTMLElement>(".ledger-row")).filter((r) => {
        const b = r.getBoundingClientRect();
        return !ledger || (b.bottom > ledger.top && b.top < ledger.bottom);
      }).slice(0, 48);
      const parts = Array.from(stage.querySelectorAll<HTMLElement>(".vault-heading, .vault-actions, .tools-row, .vault .search, .import-start, .session-grants"));
      flip([...parts, ...rows], set, 460);
    };
    apply(false);
    const watch = new ResizeObserver(() => { if (!frame) frame = requestAnimationFrame(() => apply(true)); });
    watch.observe(stage);
    return () => { watch.disconnect(); cancelAnimationFrame(frame); delete stage.dataset.shape; };
  }, []);
}

function Home({ view, run, onSend, onSettings, onCode, onCard }: { onCard: () => void; view: View; run: Run; onSend: () => void; onSettings: () => void; onCode: () => void }) {
  const n = view.keys.length;
  const heading = useRef<HTMLDivElement>(null);
  useVaultShape(heading);
  return (
    <>
      <div className="vault-heading" ref={heading}>
        <VaultMark />
        <div><h1 className="display sm">Your secure keys</h1>
        <p className="lede">{n === 0 ? "Keep your first key on this machine." : summary(view.keys)}</p></div>
      </div>
      <div className="btn-row vault-actions">
        <button className="btn primary grow" onClick={() => void run(api.add())}><Plus size={17} aria-hidden />Add a key</button>
        <button className="btn secondary grow" onClick={onSend}><Send size={16} aria-hidden />Send a key</button>
      </div>
      <div className="tools-row">
          <button className="quiet" onClick={() => void run(api.openFile())}><FolderOpen size={15} aria-hidden />Open file</button>
          {n > 0 && (<>
          <button className="quiet" onClick={() => void run(api.importEnv())}>
            <FileInput size={15} strokeWidth={1.75} aria-hidden /> Import .env
          </button>
          <button className="quiet" onClick={onCode}>
            <SquareTerminal size={15} strokeWidth={1.75} aria-hidden /> Use in code
          </button>
          </>)}
        </div>
      {view.session_grants.length > 0 && (
        <section className="session-grants" aria-label="Active grants">
          {view.session_grants.map((grant) => (
            <details className="session-grant" key={grant.id}>
              <summary><strong>{grant.session}</strong> · Review access</summary>
              <p>{grant.keys.join(", ")} in <span className="mono">{grant.folder}</span></p>
              <p>Allowed until {new Date(grant.until * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</p>
              <strong>Recent commands</strong>
              <ol>{grant.recent.map((command, index) => <li className="mono" key={index}>{command}</li>)}</ol>
              <button className="btn secondary small" onClick={() => void run(api.revokeSession(grant.id))}>Revoke access</button>
            </details>
          ))}
        </section>
      )}
      <Vault keys={view.keys} run={run} />
      <Me onCard={onCard} name={view.name} onSettings={onSettings} />
    </>
  );
}

/** "3 keys · 1 expires this week": the count, and the one thing that needs you, if any. */
function summary(keys: KeyView[]): string {
  const now = Date.now() / 1000;
  const soon = keys.filter((k) => k.expires_at !== null && k.expires_at - now < 7 * 24 * 60 * 60).length;
  const once = keys.filter((k) => k.keep === "once").length;
  const parts = [`${keys.length} ${keys.length === 1 ? "key" : "keys"} sealed on this machine`];
  if (soon > 0) parts.push(`${soon} ${soon === 1 ? "expires" : "expire"} this week`);
  else if (once > 0) parts.push(`${once} ${once === 1 ? "opens" : "open"} once`);
  return parts.join(" · ");
}

const SEARCH_ID = "key-search";
type Filter = "all" | "others" | "yours" | "expiring";
type Order = "newest" | "az" | "expiring";

const FILTERS: { id: Filter; label: string; test: (k: KeyView) => boolean }[] = [
  { id: "all", label: "All", test: () => true },
  { id: "others", label: "Shared", test: (k) => k.from !== null },
  { id: "yours", label: "Yours", test: (k) => k.from === null },
  { id: "expiring", label: "Expiring", test: (k) => k.keep !== "keep" },
];
const ORDER_LABEL: Record<Order, string> = { newest: "Newest", az: "A–Z", expiring: "Expiring first" };

/** Once-only keys first, then the soonest to expire, then kept keys. */
function expiryRank(k: KeyView): number {
  if (k.keep === "once") return -1;
  return k.expires_at ?? Number.MAX_SAFE_INTEGER;
}

const COMPARE: Record<Order, (a: KeyView, b: KeyView) => number> = {
  newest: (a, b) => b.added_at - a.added_at,
  az: (a, b) => a.label.localeCompare(b.label, undefined, { sensitivity: "base", numeric: true }),
  expiring: (a, b) => expiryRank(a) - expiryRank(b),
};

function Vault({ keys, run }: { keys: KeyView[]; run: Run }) {

  const [removing, setRemoving] = useState<KeyView | null>(null);
  // Typing a key into another app's field (Linux X11 only for now).
  const [typing, setTyping] = useState(false);
  useEffect(() => {
    void typingApi.available().then(() => setTyping(true)).catch(() => setTyping(false));
  }, []);
  // The key whose servers are being typed, and what's typed so far.
  const [hosting, setHosting] = useState<{ id: string; draft: string } | null>(null);
  const saveHosts = () => {
    if (hosting) void run(api.setHosts(hosting.id, hosting.draft).then(() => setHosting(null)));
  };
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [order, setOrder] = useState<Order>("newest");
  const ledger = useRef<HTMLDivElement>(null);
  const placed = useRef(new Map<string, DOMRect>());
  const q = query.trim().toLowerCase();
  const pass = FILTERS.find((f) => f.id === filter)!.test;
  const shown = keys.filter((key) => pass(key) &&
    [key.label, key.from, key.team, ...key.hosts].some((field) => field?.toLowerCase().includes(q))).sort(COMPARE[order]);
  const shownIds = shown.map((k) => k.id).join(",");
  useLayoutEffect(() => {
    const rows = Array.from(ledger.current?.querySelectorAll<HTMLElement>(".ledger-row") ?? []);
    const before = placed.current;
    const now = new Map<string, DOMRect>();
    for (const row of rows) {
      const id = row.dataset.id!;
      const b = row.getBoundingClientRect();
      now.set(id, b);
      const a = before.get(id);
      if (before.size === 0) continue;
      if (!a) { void enter(row, "fade", 260); continue; }
      const dx = a.left - b.left, dy = a.top - b.top;
      if (Math.abs(dx) > 1 || Math.abs(dy) > 1) row.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "translate(0, 0)" }], { duration: 340, easing: "cubic-bezier(.22,1,.36,1)" });
    }
    placed.current = now;
  }, [shownIds]);
  if (keys.length === 0) {
    return <ImportStart run={run} />;
  }
  return (
    <div className="vault">
      <div className="search">
        <Icon name="search" />
        <input
          id={SEARCH_ID}
          type="search"
          value={query}
          placeholder={`Search ${keys.length} ${keys.length === 1 ? "key" : "keys"}`}
          aria-label="Search keys"
          spellCheck={false}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setQuery("");
              e.currentTarget.blur();
            }
          }}
        />
        <details className="sort-menu" onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget)) event.currentTarget.open = false;
        }} onKeyDown={(event) => { if (event.key === "Escape") event.currentTarget.open = false; }}>
          <summary>{filter === "all" ? ORDER_LABEL[order] : FILTERS.find(f => f.id === filter)?.label}<ChevronDown size={14}/></summary>
          <div className="sort-options">
            <label>Sort<select value={order} onChange={e => setOrder(e.target.value as Order)}>
              {Object.entries(ORDER_LABEL).map(([id, label]) => <option key={id} value={id}>{label}</option>)}
            </select></label>
            <label>Show<select value={filter} onChange={e => setFilter(e.target.value as Filter)}>
              {FILTERS.map(f => <option key={f.id} value={f.id}>{f.label}</option>)}
            </select></label>
          </div>
        </details>
      </div>
      {shown.length === 0 && <p className="help no-match">No keys match{q ? ` \u201c${query.trim()}\u201d` : ""}.</p>}
      <div className="key-ledger" ref={ledger} style={{ "--ledger-height": `calc(${shown.length} * var(--row-height, 38px))` } as React.CSSProperties}>
        {shown.map(k => (
          <div className="ledger-row" key={k.id} data-id={k.id}>
            <span className="ledger-key" title={describe(k)}><KeyRound size={15} aria-hidden /></span>
            <span className="ledger-text">
              <span className="ledger-name mono" title={`${describe(k)} · ${lifetime(k)}${k.hosts.length ? " · " + k.hosts.join(", ") : ""}`}>{k.label}</span>
              <span className="ledger-meta">{k.from ? `From ${k.from}` : "Yours"} · {lifetime(k)}{k.hosts.length ? ` · ${k.hosts.join(", ")}` : ""}</span>
            </span>
            <div className="ledger-tools">
              <button title={k.hosts.length ? `Edit allowed servers: ${k.hosts.join(", ")}` : "Set allowed server"} aria-label={`Set allowed server for ${k.label}`}
                onClick={() => setHosting({ id: k.id, draft: k.hosts.join(", ") })}><Server /></button>
              {k.from === null && <button title="Replace stored value" aria-label={`Replace stored value for ${k.label}`} onClick={() => void run(api.replace(k.id))}><RefreshCw /></button>}
              {typing && <button title="Type into a field without using the clipboard" aria-label={`Type ${k.label}`} onClick={() => void run(typingApi.start(k.id))}><Keyboard /></button>}
              <button title="Copy .env pointer" aria-label={`Copy pointer for ${k.label}`} onClick={() => void run(api.copyPointer(k.id))}><Copy /></button>
              <button title="Remove from vault" aria-label={`Remove ${k.label} from vault`} onClick={() => setRemoving(k)}><X /></button>
            </div>
          </div>
        ))}
      </div>
      <Presence show={Boolean(hosting || removing)} fx="pop" inMs={240} outMs={200}>{(hosting || removing) && <VaultDialog onClose={() => { setHosting(null); setRemoving(null); }}>
        {hosting ? <form onSubmit={e => { e.preventDefault(); saveHosts(); }}>
          <h2 className="title">Allowed servers</h2>
          <p className="mono">{keys.find(k => k.id === hosting.id)?.label}</p>
          <label>Server names, separated by commas<input className="name-in" autoFocus value={hosting.draft} placeholder="api.example.com"
            onChange={e => setHosting({ ...hosting, draft: e.target.value })} spellCheck={false} /></label>
          <div className="btn-row"><button type="button" className="btn secondary" onClick={() => setHosting(null)}>Cancel</button><button className="btn primary grow">Save servers</button></div>
        </form> : removing && <>
          <h2 className="title">Remove from vault?</h2><p className="mono">{removing.label}</p>
          <div className="btn-row"><button autoFocus className="btn secondary grow" onClick={() => setRemoving(null)}>Cancel</button>
            <button className="btn secondary" onClick={() => void run(api.forget(removing.id).then(() => setRemoving(null)))}>Remove key</button></div>
        </>}
      </VaultDialog>}</Presence>
    </div>
  );
}

/** A modal dialog. Escape (and, with `light`, a click outside) asks to
 * close rather than closing at once, so Presence can ease it out. */
function VaultDialog({ children, onClose, className = "", light = false }: { children: React.ReactNode; onClose: () => void; className?: string; light?: boolean }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { const el = dialog.current; el?.showModal(); return () => el?.close(); }, []);
  return <dialog className={`vault-dialog ${className}`} ref={dialog}
    onCancel={e => { e.preventDefault(); onClose(); }}
    onClick={e => { if (light && e.target === e.currentTarget) onClose(); }}>{children}</dialog>;
}

/** Clicking the logo: the version and what changed, newest first. */
function HistoryDialog({ onClose }: { onClose: () => void }) {
  const [version, setVersion] = useState("");
  useEffect(() => { void getVersion().then(setVersion, () => undefined); }, []);
  const day = (d: string) => new Date(`${d}T12:00:00`).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  return <VaultDialog className="history" light onClose={onClose}>
    <div className="history-inner">
      <header className="history-head">
        <Logo height={30} />
        <div><h2>KeyHandoff</h2><p>{version ? `Version ${version}` : " "}</p></div>
        <button className="history-close" aria-label="Close" onClick={onClose}><X size={16} /></button>
      </header>
      <ol className="history-list">
        {HISTORY.map((r, i) => <li key={r.date} className={i === 0 ? "latest" : ""}>
          <time dateTime={r.date}>{day(r.date)}</time>
          <h3>{r.title}{i === 0 && <span className="history-tag">Latest</span>}</h3>
          <ul>{r.notes.map(n => <li key={n}>{n}</li>)}</ul>
        </li>)}
      </ol>
    </div>
  </VaultDialog>;
}

function Me({ name, onSettings, onCard }: { name: string; onSettings: () => void; onCard: () => void }) {
  return <footer className="identity-dock">
    <button className="pocket-card" onClick={onCard}><IdCard aria-hidden /><span>Share my card<small>.handoff file</small></span><ArrowUpRight size={14} aria-hidden /></button>
    <strong className="dock-name" title={name}>{name}</strong>
    <button className="dock-settings" title="Settings" aria-label="Settings" onClick={onSettings}><Settings /></button>
  </footer>;
}

function SettingsSheet({ view, run, onSecurity, onClose }: { view: View; run: Run; onSecurity: () => void; onClose: () => void }) {
  const [name, setName] = useState(view.name);
  const [version, setVersion] = useState("");
  useEffect(() => { void run(getVersion().then(setVersion)); }, []);
  return <section className="sheet preferences" role="dialog" aria-labelledby="settings-title">
    <h2 className="display md" id="settings-title">Settings</h2>
    <form onSubmit={e => { e.preventDefault(); void run(api.setName(name.trim())); }}>
      <label htmlFor="card-name">Name on your receiving card</label>
      <div className="btn-row"><input id="card-name" className="name-in" value={name} maxLength={64} onChange={e => setName(e.target.value)}/>
        <button className="btn secondary" disabled={!name.trim() || name.trim() === view.name}>Save</button></div>
    </form>
    <button className="pref-link" onClick={onSecurity}><LockKeyhole size={20}/>Vault security<ArrowUpRight size={16}/></button>
    <button className="pref-link about-site" onClick={() => void run(api.openSite("keyhandoff"))}>
      <Logo /><span>KeyHandoff<small>keyhandoff.dev{version && ` · Version ${version}`}</small></span><ArrowUpRight size={16}/></button>
    <button className="pref-link about-site" onClick={() => void run(api.openSite("maker"))}>
      <MakerMark height={24} /><span>Made by Strange Systems<small>strangesystems.dev</small></span><ArrowUpRight size={16}/></button>
    <span className="grow-space"/><button className="btn secondary" onClick={onClose}>Done</button>
  </section>;
}

const KEEP_HELP: Record<Keep, string> = {
  once: "Opens one time, then it's destroyed.",
  days30: "Destroyed automatically in 30 days.",
  keep: "Kept until you remove it.",
};

/** Opening a sealed key: it unlocks itself, then one tap picks how long to keep it. */
/** The Lock after choices, in minutes; null is Never. */
const LOCK_AFTER: [number | null, string][] = [[null, "Never"], [15, "15 minutes"], [60, "1 hour"]];

/** How the vault opens: a TPM PIN, the passphrase, or remembered here. */
function SecuritySheet({ view, run, onClose }: { view: View; run: Run; onClose: () => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  const [pick, setPick] = useState(view.remembered);
  const [adding, setAdding] = useState(false);
  const [pin, setPin] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  useEffect(() => void enter(sheet.current, "rise", 360), []);
  const save = () => void run(gateApi.remember(pick)).then(onClose);
  const addPin = async () => {
    setBusy(true);
    await run(gateApi.addPin(pin));
    setBusy(false);
    setAdding(false);
    setPin("");
    setAgain("");
  };
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph mark="key" />
        <div>
          <span className="lede">Security</span>
          <h2 className="display md">How it opens</h2>
        </div>
      </div>
      <div className="keep-buttons two">
        <button className={pick ? "" : "pick"} onClick={() => setPick(false)}>
          <Icon name="lock" /> Ask to unlock
        </button>
        <button className={pick ? "pick" : ""} onClick={() => setPick(true)}>
          <Icon name="key" /> Remember here
        </button>
      </div>
      {pick ? (
        <div className="warning" role="alert">
          <Icon name="alert" />
          <div>
            <b>Any program running as you could open your keys.</b>
            <p>
              KeyHandoff opens without asking, and stays open when the computer sleeps or the screen locks. Your
              computer's sign-in is the only lock left. Your passphrase and recovery code still work.
            </p>
          </div>
        </div>
      ) : (
        <>
          <p className="help">
            KeyHandoff asks when it starts, and again after the computer sleeps or the screen locks. While it's
            locked, no other program can open your keys.
          </p>
          <div className="pin-block">
            <span className="label">Lock after</span>
            <div className="keep-buttons" role="group" aria-label="Lock after">
              {LOCK_AFTER.map(([minutes, label]) => (
                <button key={label} className={view.lock_after === minutes ? "pick" : ""} onClick={() => void run(gateApi.setLockAfter(minutes))}>
                  {label}
                </button>
              ))}
            </div>
            <p className="help">How long KeyHandoff stays open while you're not using it.</p>
          </div>
          <div className="pin-block">
            <span className="label">PIN instead of the passphrase</span>
            {view.tpm === "missing" && (
              <p className="help">
                This needs a TPM, and none is turned on here. Many computers have one switched off in the firmware
                settings (AMD fTPM or Intel PTT).
              </p>
            )}
            {view.tpm === "no-access" && (
              <p className="help">
                This computer has a TPM, but KeyHandoff isn't allowed to use it yet. Run{" "}
                <span className="mono">sudo usermod -aG tss $USER</span>, then sign out and back in.
              </p>
            )}
            {view.tpm === "ready" && view.pin && !adding && (
              <p className="help">
                On. This computer's TPM checks the PIN and stops anyone guessing it.{" "}
                <button className="link inline" onClick={() => void run(gateApi.removePin())}>Turn off</button>
              </p>
            )}
            {view.tpm === "ready" && !view.pin && !adding && (
              <p className="help">
                A short PIN, checked by this computer's TPM, which stops anyone guessing it.{" "}
                <button className="link inline" onClick={() => setAdding(true)}>Add a PIN</button>
              </p>
            )}
            {adding && (
              <div className="pin-add">
                <input className="name-in" type="password" autoFocus value={pin} placeholder="PIN" aria-label="PIN" onChange={(e) => setPin(e.target.value)} />
                <input className="name-in" type="password" value={again} placeholder="Again" aria-label="PIN again" onChange={(e) => setAgain(e.target.value)} />
                <button className="btn secondary small" disabled={busy || pin.trim().length < 4 || pin !== again} onClick={() => void addPin()}>
                  {busy ? "Adding…" : "Add"}
                </button>
              </div>
            )}
          </div>
        </>
      )}
      <div className="pin-block">
        <span className="label">Backup</span>
        <p className="help">
          A file that opens only with your recovery code, so it's safe on a thumb drive or in cloud storage. Restore
          it when you set up KeyHandoff on a new computer.{" "}
          <button className="link inline" onClick={() => void run(gateApi.saveBackup().then(setSaved))}>
            {saved ? "Saved. Save another" : "Save a backup"}
          </button>
        </p>
      </div>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={onClose}>{pick === view.remembered ? "Done" : "Cancel"}</button>
        <button className="btn primary grow" disabled={pick === view.remembered} onClick={save}>
          {pick ? "Remember on this computer" : "Ask to unlock"}
        </button>
      </div>
    </div>
  );
}

function AcceptSheet({ item, run }: { item: KeyView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [item.id]);
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph mark="check" />
        <div>
          <span className="lede">
            {item.from ? `${item.from} sent you a key` : "Your key"}
            {item.team ? ` for ${item.team}` : ""}
          </span>
          <h2 className="display md">{item.label}</h2>
        </div>
      </div>
      <div className="keep-block">
        <div className="keep-buttons" role="group" aria-label="How long to keep it">
          {KEEP_CHOICES.map((c) => (
            <button
              key={c.keep}
              className={item.keep === c.keep ? "pick" : ""}
              aria-pressed={item.keep === c.keep}
              onClick={() => void run(api.setKeep(item.id, c.keep).then(api.dismiss))}
            >
              <Icon name={c.icon} />
              {c.label}
            </button>
          ))}
        </div>
        <p className="help">{KEEP_HELP[item.keep]}</p>
      </div>
      <p className="help">It's in your keys. The copy in the chat can't be opened again.</p>
      <span className="grow-space" />
      <button className="btn primary" onClick={() => void api.dismiss()}>Done</button>
    </div>
  );
}

/**
 * A program asks for kept keys, through a `handoff:` pointer or `keyhandoff run`.
 * The launch origin determines which approvals the sheet offers.
 */
function ProgramSheet({ program, run }: { program: ProgramView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [program.path, program.folder, program.command]);
  const one = program.keys.length === 1;
  const agent = program.agent;
  const [background, setBackground] = useState(false);
  useEffect(() => setBackground(false), [program.id]);
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph base="window" mark="key" />
        <div>
          <span className="lede">{agent ? `${agent} wants` : "A program wants"} {one ? "a key" : `${program.keys.length} keys`}</span>
          <h2 className="display md">{program.name}</h2>
        </div>
      </div>
      <div className="facts">
        <div className="fact" title="The exact command">
          <SquareTerminal size={17} strokeWidth={1.75} aria-hidden />
          <span className="mono">{program.command ?? program.name}</span>
        </div>
        <div className="fact" title="The folder it runs in">
          <Folder size={17} strokeWidth={1.75} aria-hidden />
          <span className="mono">{program.folder ?? "a folder KeyHandoff can't see"}</span>
        </div>
        <div className="fact on" title="The keys it asks for">
          <KeyRound size={17} strokeWidth={1.75} aria-hidden />
          <span>{program.keys.join(", ")}</span>
        </div>
      </div>
      <p className="help">
        {program.unattended ? (
          program.extends
            ? `Started by ${program.unattended}. You allowed this exact command in this folder yourself. Let it run in the background from now on? A changed script asks again.`
            : `Started by ${program.unattended}. Approval covers this request only. To let it run on its own, run this exact command once yourself in this folder and choose Always allow, also in the background.`
        ) : agent ? (
          <><Bot size={13} strokeWidth={2} aria-hidden className="inline-ico" /> {program.session
            ? program.session + ". Allow any command from this session to use the listed keys in this folder for 30 minutes."
            : agent + " started this command. KeyHandoff could not verify a dedicated session. Approval covers this request only."}</>
        ) : (
          "Always covers this exact command in this folder. A changed script asks again."
        )}
      </p>
      {!agent && !program.unattended && (
        <label className="help bg-check">
          <input type="checkbox" checked={background} onChange={(e) => setBackground(e.target.checked)} />{" "}
          With Always, also when it runs in the background (a timer, cron, a service)
        </label>
      )}
      {program.session_image && (
        <details className="help"><summary>Agent executable</summary><span className="mono">{program.session_image}</span></details>
      )}
      {program.session && (
        <button className="quiet" onClick={() => void run(api.answer(program.id, "30min"))}>Allow only this command for 30 min</button>
      )}
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void run(api.answer(program.id, "deny"))}>Deny</button>
        <button className="btn secondary" onClick={() => void run(api.answer(program.id, "once"))}>Allow once</button>
        {program.session && (
          <button className="btn primary grow" onClick={() => void run(api.answer(program.id, "session"))}>Allow for 30 minutes</button>
        )}
        {!agent && !program.unattended && (
          <button className="btn primary grow" onClick={() => void run(api.answer(program.id, background ? "always-background" : "always"))}>Always allow</button>
        )}
        {!agent && program.unattended && program.extends && (
          <button className="btn primary grow" onClick={() => void run(api.answer(program.id, "always-background"))}>Allow in the background</button>
        )}
      </div>
    </div>
  );
}

/** A program under `keyhandoff run` tried to send a key's stand-in somewhere else, and was stopped. */
function AlertSheet({ alert }: { alert: AlertView }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [alert.at, alert.destination]);
  const teams = alert.shared_with;
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph base="window" mark="stop" />
        <div>
          <span className="lede">KeyHandoff stopped a leak</span>
          <h2 className="display md">{alert.key}</h2>
        </div>
      </div>
      <div className="chip">
        <Icon name="file" />
        <span className="mono">{alert.command}</span>
      </div>
      <p className="lede">
        It tried to send {alert.key}'s stand-in to <strong>{alert.destination}</strong>. Nothing left your computer, and the stand-in was never the key.
      </p>
      <p className="help">
        From now on, programs you start with keyhandoff run can't reach {alert.destination}
        {teams.length ? `, and neither can anyone on ${teams.join(", ")}` : ""}. If you didn't expect this, look at what{" "}
        <span className="mono">{alert.command}</span> runs.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn primary grow" autoFocus onClick={() => void api.dismiss()}>Done</button>
      </div>
    </div>
  );
}

/** Your card is on the clipboard; it can also be dragged out. */
function CardSheet({ fileName, computer, run }: { fileName: string; computer: string; run: Run }) {
  const sheet = useRef<HTMLElement>(null);
  const drag = useDragOut(() => void run(api.dragOut("request")));
  useEffect(() => void enter(sheet.current, "rise", 360), [fileName]);
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);
  return (
    <section className="sheet receiving-sheet" ref={sheet} role="dialog" aria-labelledby="receiving-title">
      <h2 className="display md" id="receiving-title">Share your card</h2>
      <p className="lede">Give this file to someone so they can send you keys.</p>
      <div className="card-file" {...drag} title="Drag this card into a folder or message">
        <IdCard size={32}/><strong>{fileName}</strong><small>Drag card file</small>
      </div>
      <button className="btn primary" onClick={() => void run(api.saveCard().then(ok => { if (ok) setSaved(true); }))}>
        {saved ? <Check size={17}/> : <Download size={17}/>} {saved ? "Save another copy" : "Save card file"}
      </button>
      <button className="quiet" onClick={() => void run(api.copyRequest().then(() => setCopied(true)))}>{copied ? "Copied" : "Copy card instead"}</button>
      <details className="help"><summary>Card details</summary><p>For one recipient, up to 12 keys. Computer fingerprint: <span className="mono">{computer}</span>.</p></details>
      <span className="grow-space"/><button className="btn secondary" onClick={() => void run(api.dismiss())}>Done</button>
    </section>
  );
}

/** Someone's card was just opened: they're a contact now. */
function ContactSheet({ contact, onSend }: { contact: ContactView; onSend: (c: ContactView) => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [contact.id]);
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph mark="check" />
        <div>
          <span className="lede">Card added</span>
          <h2 className="display md">{contact.name}</h2>
        </div>
      </div>
      <p className="lede">You can send {contact.name} up to {contact.sends_left} keys with this card, any time.</p>
      <p className="help">
        {contact.name}'s computer is <span className="mono">{contact.computer}</span>. If it matters, check it with them.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void api.dismiss()}>Done</button>
        <button className="btn primary grow" onClick={() => onSend(contact)}>
          Send {contact.name} a key
        </button>
      </div>
    </div>
  );
}

/** At this many sends left or fewer, Send a key marks the contact: time to ask for a new card. */
const LOW_SENDS = 2;
const runningLow = (c: ContactView) => c.sends_left <= LOW_SENDS;
const sendsLeft = (c: ContactView) => (c.sends_left === 0 ? "card used up" : `${c.sends_left} ${c.sends_left === 1 ? "send" : "sends"} left`);

/** Who Send a key sends to: a contact or a team. */
/** `folder`: a team folder you keep; the key goes into its next version. */
type To = { team: boolean; id: string; name: string; folder?: boolean };

/** Send a key: pick a person or a team (unless `start` picked one), then one of your saved keys. */
function SendSheet({ view, run, start, onEdit, onClose }: { view: View; run: Run; start: To | null; onEdit: (t: TeamView | "new") => void; onClose: () => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  const [to, setTo] = useState<To | null>(start);
  const [picked, setPicked] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The contact whose Forget was clicked once; a second click forgets them.
  const [armed, setArmed] = useState<string | null>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [to === null]);
  const { contacts, teams } = view;
  if (to) {
    const selected = view.keys.find((k) => k.id === picked);
    const send = (key: KeyView) => {
      setBusy(true);
      const action = to.folder ? api.shareWithFolderTeam(to.id, key.id) : to.team ? api.sendKeptToTeam(to.id, key.id) : api.sendKept(to.id, key.id);
      void run(action.then(onClose)).finally(() => setBusy(false));
    };
    return (
      <div className="sheet" ref={sheet}>
        <div className="hero">
          <h2 className="display md">Send to {to.name}</h2>
          <p className="lede">{view.keys.length ? "Which key?" : "You have no keys to send yet."}</p>
        </div>
        {view.keys.length ? (
          <div className="keys">
            {view.keys.map((k) => (
              <button key={k.id} className="key-row contact send-choice" aria-pressed={picked === k.id} onClick={() => setPicked(k.id)}>
                <span className="key-ico"><Icon name="key" /></span>
                <span className="who">
                  <span className="nm">{k.label}</span>
                  <span className="meta">
                    {k.from ? `From ${k.from}` : "Yours"}
                    {k.team ? ` · ${k.team}` : ""}
                  </span>
                </span>
              </button>
            ))}
          </div>
        ) : (
          <p className="help">Use <b>Add a key</b> on the home screen to keep a key first. Then you can send it to anyone on your list.</p>
        )}
        <span className="grow-space" />
        <div className="btn-row">
          <button className="btn secondary" disabled={busy} onClick={() => { setPicked(null); setTo(null); }}>Back</button>
          {selected && <button className="btn primary grow send-confirm" disabled={busy} onClick={() => send(selected)}>
            {to.folder ? "Share" : "Seal"} {selected.label} {to.folder ? "with" : "for"} {to.name}
          </button>}
        </div>
      </div>
    );
  }
  return (
    <div className="sheet" ref={sheet}>
      <div className="hero">
        <h2 className="display md">Send a key</h2>
        <p className="lede">{contacts.length ? "Who's it for?" : "Nobody to send to yet."}</p>
      </div>
      <button
        className="chip nearby-toggle"
        aria-pressed={view.nearby}
        title="Let people on this network find this computer. Only people on your list can send you keys."
        onClick={() => void run(api.setNearby(!view.nearby))}
      >
        <Icon name={view.nearby ? "check" : "users"} />
        <span>{view.nearby ? "Nearby is on: people on this network can find you" : "Find people on this network"}</span>
      </button>
      {view.nearby && <p className="help nearby-status">{nearbyStatus(contacts, view.nearby_others)}</p>}
      <button className="chip nearby-toggle" onClick={() => void run(api.startPairing())} title="Meet someone in person: both of you choose this, then compare a code.">
        <Icon name="user" />
        <span>Add someone nearby</span>
      </button>
      {view.folder_teams.length > 0 && (
        <div className="keys">
          <span className="label">Team folders</span>
          {view.folder_teams.map((t) => (
            <FolderTeamRows key={t.id} team={t} run={run} onPick={() => setTo({ team: true, folder: true, id: t.id, name: t.name })} />
          ))}
        </div>
      )}
      {teams.length > 0 && (
        <div className="keys">
          <span className="label">Teams</span>
          {teams.map((t) => (
            <div key={t.id} className="key-row">
              <button className="who contact" onClick={() => setTo({ team: true, id: t.id, name: t.name })}>
                <span className="key-ico"><Icon name="users" /></span>
                <span className="who">
                  <span className="nm">{t.name}</span>
                  <span className="meta">
                    {t.members.map((m) => m.name).join(", ")}
                    {t.members.filter(runningLow).map((m) => (
                      <span key={m.id} className="low"> · {m.name}: {sendsLeft(m)}</span>
                    ))}
                  </span>
                </span>
              </button>
              <button className="use always" onClick={() => onEdit(t)}>Edit</button>
            </div>
          ))}
        </div>
      )}
      {contacts.length ? (
        <div className="keys">
          {teams.length > 0 && <span className="label">People</span>}
          {contacts.map((c) => (
            <div key={c.id} className="key-row">
              <button className="who contact" onClick={() => setTo({ team: false, id: c.id, name: c.name })}>
                <span className="key-ico"><Icon name="user" /></span>
                <span className="who">
                  <span className="nm">{c.name}</span>
                  <span className="meta">
                    {c.nearby && <span className="near">Nearby · </span>}
                    <span className={runningLow(c) ? "low" : ""}>
                      {c.sends_left > 0 ? sendsLeft(c) : "Card used up: ask for a new one"}
                    </span>
                    {" · "}
                    <span className="mono">{c.computer}</span>
                  </span>
                </span>
              </button>
              <button
                className={`forget ${armed === c.id ? "sure" : ""}`}
                aria-label={`Forget ${c.name}`}
                title={`Forget ${c.name}. Keys you already sent stay with them.`}
                onClick={() => {
                  if (armed === c.id) void run(api.forgetContact(c.id));
                  else setArmed(c.id);
                }}
                onBlur={() => setArmed(null)}
              >
                {armed === c.id ? "Forget for good?" : <Icon name="close" />}
              </button>
            </div>
          ))}
        </div>
      ) : (
        <p className="help">
          Ask the person for their KeyHandoff card, then open it here: double-click it, drop it on this window, or paste it with
          Ctrl V.
        </p>
      )}
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary grow" onClick={onClose}>Close</button>
        <button className="btn secondary grow" onClick={() => onEdit("new")}>
          <Icon name="users" /> New team
        </button>
      </div>
    </div>
  );
}

/** A team folder in Send a key: its row, then anyone asking to join it, for the keeper to add. */
function FolderTeamRows({ team, run, onPick }: { team: FolderTeamView; run: Run; onPick: () => void }) {
  // The person whose cross was clicked once; a second click removes them.
  const [armed, setArmed] = useState<string | null>(null);
  const meta = team.waiting
    ? `Waiting for ${team.keeper} to add you`
    : team.members.length
      ? team.members.join(", ")
      : team.keeping
        ? "No one yet. People join from invite.handoff in the folder."
        : `Kept by ${team.keeper}`;
  return (
    <>
      <div className="key-row" title={team.folder}>
        {/* The keeper picks the team to share a key with it; members can't. */}
        <button className="who contact" disabled={!team.keeping || team.members.length === 0} onClick={onPick}>
          <span className="key-ico"><Icon name="users" /></span>
          <span className="who">
            <span className="nm">{team.name}</span>
            <span className="meta">{meta}</span>
          </span>
        </button>
      </div>
      {team.to_replace.length > 0 && (
        <div className="warning">
          <Icon name="alert" />
          <div>
            <b>Replace with the provider: {team.to_replace.join(", ")}</b>
            <p>Someone who left still has these. Make new ones, then click Replace on each, and the team gets them.</p>
          </div>
        </div>
      )}
      {team.people.map((p) => (
        <div key={p.id} className="key-row">
          <span className="key-ico"><Icon name="user" /></span>
          <span className="who">
            <span className="nm">{p.name}</span>
            <span className="meta mono">{p.computer}</span>
          </span>
          <button
            className={`forget ${armed === p.id ? "sure" : ""}`}
            aria-label={`Take ${p.name} off ${team.name}`}
            title={`Take ${p.name} off ${team.name}. They get nothing new, and you see which keys to replace.`}
            onClick={() => (armed === p.id ? void run(api.removeFromTeam(team.id, p.id)) : setArmed(p.id))}
            onBlur={() => setArmed(null)}
          >
            {armed === p.id ? `Take off ${team.name}?` : <Icon name="close" />}
          </button>
        </div>
      ))}
      {team.asking.map((p) => (
        <div key={p.id} className="key-row">
          <span className="key-ico on"><Icon name="user" /></span>
          <span className="who">
            <span className="nm">{p.name} wants to join</span>
            <span className="meta">
              Their computer is <span className="mono">{p.computer}</span>
            </span>
          </span>
          <button className="use always" onClick={() => void run(api.turnAway(team.id, p.id))}>Not now</button>
          <button className="use always" onClick={() => void run(api.addToTeam(team.id, p.id))}>Add</button>
        </div>
      ))}
    </>
  );
}

/** What Nearby can see right now, and what to do about it. */
export function nearbyStatus(contacts: ContactView[], others: number): string {
  const near = contacts.filter((c) => c.nearby).map((c) => c.name);
  if (!near.length && !others) return "No one else on this network has Nearby on yet. Ask them to open KeyHandoff and turn it on.";
  const parts: string[] = [];
  if (near.length) parts.push(`${near.join(", ")} ${near.length === 1 ? "is" : "are"} nearby: pick them below to send straight to their computer.`);
  if (others) {
    const n = others === 1 ? "1 other computer here has" : `${others} other computers here have`;
    parts.push(`${n} Nearby on. To add them, both of you choose Add someone nearby.`);
  }
  return parts.join(" ");
}

/** Adding someone nearby: pick them, then compare the code on both screens. */
function PairingSheet({ pairing, run }: { pairing: PairingView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), []);
  const cancel = () => void run(api.cancelPairing());
  if (pairing.with && pairing.code) {
    return (
      <div className="sheet" ref={sheet}>
        <div className="hero">
          <span className="lede">Compare with {pairing.with}</span>
          <h2 className="display pairing-code mono">{pairing.code}</h2>
        </div>
        <p className="lede">Does {pairing.with}'s screen show the same code? If it doesn't, someone may be in the middle: choose No.</p>
        <span className="grow-space" />
        <div className="btn-row">
          <button className="btn secondary" onClick={cancel}>No</button>
          <button className="btn primary grow" onClick={() => void run(api.confirmPairing())}>Yes, add {pairing.with}</button>
        </div>
      </div>
    );
  }
  return (
    <div className="sheet" ref={sheet}>
      <div className="hero">
        <h2 className="display md">Add someone nearby</h2>
        <p className="lede">
          {pairing.with ? `Waiting for ${pairing.with}'s card…` : "Ask them to choose Add someone nearby too, then pick them here."}
        </p>
      </div>
      <div className="keys">
        {pairing.people.length === 0 && <p className="help">No one nearby is adding people yet.</p>}
        {pairing.people.map((p) => (
          <div key={p.id} className="key-row">
            <button className="who contact" disabled={pairing.with !== null} onClick={() => void run(api.pairWith(p.id))}>
              <span className="key-ico"><Icon name="user" /></span>
              <span className="who">
                <span className="nm">{p.name}</span>
                <span className="meta mono">{p.computer}</span>
              </span>
            </button>
          </div>
        ))}
      </div>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary grow" onClick={cancel}>Cancel</button>
      </div>
    </div>
  );
}

/** A team invite you opened: who keeps the team, and Join. */
function InviteSheet({ invite, run }: { invite: InviteView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [invite.team]);
  const join = () =>
    void run(
      invite.folder
        ? api.joinTeam(null)
        : api.pickFolder().then((folder) => (folder ? api.joinTeam(folder) : undefined)),
    );
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph mark="send" />
        <div>
          <span className="lede">{invite.keeper} invited you to</span>
          <h2 className="display md">{invite.team}</h2>
        </div>
      </div>
      <p className="lede">
        {invite.folder
          ? `You'll join through this folder, and ${invite.keeper} adds you from there.`
          : `Pick your copy of the team's shared folder. ${invite.keeper} adds you from there.`}
      </p>
      {invite.folder && <p className="info mono">{invite.folder}</p>}
      <p className="help">
        {invite.keeper}'s computer is <span className="mono">{invite.computer}</span>. If it matters, check it with them.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void run(api.closeInvite())}>Not now</button>
        <button className="btn primary grow" onClick={join}>
          {invite.folder ? `Join ${invite.team}` : "Pick folder and join"}
        </button>
      </div>
    </div>
  );
}

/**
 * Make or change a team: a name and some of your contacts. Welcome sends a
 * member every key you hold that was shared with the team.
 */
function TeamSheet({ team, contacts, run, onClose }: { team: TeamView | null; contacts: ContactView[]; run: Run; onClose: () => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  const [name, setName] = useState(team?.name ?? "");
  const [picked, setPicked] = useState<string[]>(team?.members.map((m) => m.id) ?? []);
  useEffect(() => void enter(sheet.current, "rise", 360), []);
  const saved = new Set(team?.members.map((m) => m.id) ?? []);
  const toggle = (id: string) => setPicked(picked.includes(id) ? picked.filter((p) => p !== id) : [...picked, id]);
  const save = () => void run(api.saveTeam(team?.id ?? null, name, picked).then(onClose));
  // A new team can live in a folder the team already syncs instead.
  const shareFolder = () =>
    void run(
      api.pickFolder().then((folder) => (folder ? api.makeFolderTeam(name, folder).then(onClose) : undefined)),
    );
  return (
    <div className="sheet" ref={sheet}>
      <div className="hero">
        <h2 className="display md">{team ? team.name : "New team"}</h2>
        <p className="lede">
          {team ? `You hold ${team.keys} ${team.keys === 1 ? "key" : "keys"} shared with this team.` : "Share a key with everyone on it at once."}
        </p>
      </div>
      <input className="name-in" value={name} maxLength={64} placeholder="Team name" aria-label="Team name" onChange={(e) => setName(e.target.value)} />
      <div className="keys">
        {contacts.map((c) => (
          <div key={c.id} className="key-row">
            <button className="who contact" onClick={() => toggle(c.id)} aria-pressed={picked.includes(c.id)}>
              <span className={`key-ico ${picked.includes(c.id) ? "on" : ""}`}>
                <Icon name={picked.includes(c.id) ? "check" : "user"} />
              </span>
              <span className="who">
                <span className="nm">{c.name}</span>
                <span className="meta">{c.sends_left} sends left</span>
              </span>
            </button>
            {team && saved.has(c.id) && team.keys > 0 && (
              <button className="use always" onClick={() => void run(api.welcome(team.id, c.id))} title={`Send ${c.name} every team key you hold`}>
                Welcome
              </button>
            )}
          </div>
        ))}
      </div>
      <p className="help">
        {team || contacts.length
          ? "Everyone on the team needs to have sent you their card. Anyone can set up the same team on their computer to share too."
          : "No cards yet? Keep the team in a folder you all sync instead, and people join from there."}
      </p>
      {!team && contacts.length > 0 && (
        <button className="btn secondary" onClick={shareFolder} title="Dropbox, Google Drive, OneDrive, a repo or a network share">
          <Icon name="file" /> Keep it in a shared folder
        </button>
      )}
      <span className="grow-space" />
      <div className="btn-row">
        {team && (
          <button className="btn secondary" onClick={() => void run(api.deleteTeam(team.id).then(onClose))}>Delete</button>
        )}
        <button className="btn secondary" onClick={onClose}>Cancel</button>
        {team || contacts.length ? (
          <button className="btn primary grow" onClick={save}>Save</button>
        ) : (
          <button className="btn primary grow" onClick={shareFolder} title="Dropbox, Google Drive, OneDrive, a repo or a network share">
            Pick the shared folder
          </button>
        )}
      </div>
    </div>
  );
}

function lifetime(k: KeyView): string {
  if (k.keep === "once") return "Opens once";
  if (k.expires_at === null) return "Kept";
  const days = Math.max(0, Math.ceil((k.expires_at * 1000 - Date.now()) / DAY_MS));
  return days <= 1 ? "Goes today" : `${days} days left`;
}

function describe(k: KeyView): string {
  const added = new Date(k.added_at * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short" });
  return `${k.label}, ${k.from ? `from ${k.from}` : "added by you"} on ${added}`;
}

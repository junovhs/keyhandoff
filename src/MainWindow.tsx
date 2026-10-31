import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon, Logo, Tile, useDragOut } from "./art";
import { api, attempt, KEEP_CHOICES, useView, type Keep, type KeyView, type ProgramView, type View } from "./ipc";
import { enter, stagger } from "./motion";
import { SafeCopy } from "./SafeCopy";

const DAY_MS = 24 * 60 * 60 * 1000;

export function MainWindow() {
  const view = useView();
  const stage = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const shown = useRef(false);

  useEffect(() => {
    // The window starts hidden and is shown once React has rendered, so it
    // never flashes white. (No requestAnimationFrame here: webviews don't run
    // it while the window is hidden.)
    if (!view || shown.current) return;
    shown.current = true;
    stagger(stage.current);
    void api.ready();
  }, [view]);

  // Ctrl+V anywhere pastes a KeyHandoff file or message (Rust reads the clipboard).
  // Typing anywhere, "/" or Ctrl+F jumps to the key search.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement;
      const mod = e.ctrlKey || e.metaKey;
      if (!typing && mod && e.key.toLowerCase() === "v") {
        e.preventDefault();
        void run(api.paste());
        return;
      }
      const search = document.getElementById(SEARCH_ID);
      const covered = document.querySelector(".sheet, .safe") !== null;
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
  const toast = view?.program ? null : (error ?? view?.notice ?? null);
  const safeOn = view !== null && view.safe.state !== "off";

  return (
    <div className="app">
      <header className="titlebar" data-tauri-drag-region>
        <Logo />
        <span className="name" data-tauri-drag-region>KeyHandoff</span>
        <button aria-label="Minimize" onClick={() => void win.minimize()}>
          <Icon name="minimize" />
        </button>
        <button aria-label="Close" onClick={() => void win.close()}>
          <Icon name="close" />
        </button>
      </header>
      <main className="stage" ref={stage}>
        {view && !safeOn && <Home view={view} run={run} />}
      </main>
      {view && safeOn && <SafeCopy view={view} />}
      {view?.accepted && <AcceptSheet item={view.accepted} run={run} />}
      {view?.asked && !view.accepted && <AskSheet fileName={view.asked} computer={view.computer} run={run} />}
      {view?.program && <ProgramSheet program={view.program} run={run} />}
      {toast && (
        <button className={`toast ${error ? "bad" : ""}`} onClick={() => { setError(null); void api.dismiss(); }}>
          {!error && <Icon name="check" />}
          {toast}
        </button>
      )}
    </div>
  );
}

type Run = (p: Promise<unknown>) => Promise<void>;

function Home({ view, run }: { view: View; run: Run }) {
  const n = view.keys.length;
  const long = n >= TOOLS_FROM;
  return (
    <>
      <div className="hero">
        <h1 className={`display ${long ? "sm" : ""}`}>{n === 0 ? "No keys yet." : <><span className="count">{n}</span> {n === 1 ? "key" : "keys"}, sealed.</>}</h1>
        {n === 0 && <p className="lede">Ask for a key, or add one of your own.</p>}
      </div>
      <div className="btn-row">
        <button className="btn primary grow" onClick={() => void run(api.ask())}>
          Request a key
        </button>
        <button className="btn secondary" onClick={() => void run(api.add())}>Add your own</button>
      </div>
      <Vault keys={view.keys} run={run} />
      <Me name={view.name} pending={view.pending} run={run} />
    </>
  );
}

const SEARCH_ID = "key-search";
/** Search, filters and sort appear once the list is long enough to need them. */
const TOOLS_FROM = 6;

type Filter = "all" | "others" | "yours" | "expiring";
type Order = "newest" | "az" | "expiring";

const FILTERS: { id: Filter; label: string; test: (k: KeyView) => boolean }[] = [
  { id: "all", label: "All", test: () => true },
  { id: "others", label: "Shared", test: (k) => k.from !== null },
  { id: "yours", label: "Yours", test: (k) => k.from === null },
  { id: "expiring", label: "Expiring", test: (k) => k.keep !== "keep" },
];
const NEXT_ORDER: Record<Order, Order> = { newest: "az", az: "expiring", expiring: "newest" };
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
  const [armed, setArmed] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [order, setOrder] = useState<Order>("newest");
  if (keys.length === 0) {
    return (
      <ol className="steps">
        <li><span className="n">1</span><span><b>Request a key</b> and paste the request into a chat with whoever has it.</span></li>
        <li><span className="n">2</span><span>They send back a sealed file. Only this computer can open it.</span></li>
        <li><span className="n">3</span><span>Paste it here. The key goes into your vault and is never shown.</span></li>
      </ol>
    );
  }
  const tools = keys.length >= TOOLS_FROM;
  const q = query.trim().toLowerCase();
  const pass = FILTERS.find((f) => f.id === filter)!.test;
  const shown = tools
    ? keys
        .filter((k) => pass(k) && (!q || k.label.toLowerCase().includes(q) || (k.from ?? "").toLowerCase().includes(q)))
        .sort(COMPARE[order])
    : keys;
  return (
    <div className="vault">
      {tools && (
        <>
          <div className="search">
            <Icon name="search" />
            <input
              id={SEARCH_ID}
              type="search"
              value={query}
              placeholder={`Search ${keys.length} keys`}
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
            <button className="order" onClick={() => setOrder(NEXT_ORDER[order])} title="Change the order">
              {ORDER_LABEL[order]}
            </button>
          </div>
          <div className="chips" role="group" aria-label="Show">
            {FILTERS.map((f) => (
              <button key={f.id} className={filter === f.id ? "on" : ""} aria-pressed={filter === f.id} onClick={() => setFilter(f.id)}>
                {f.label} <span>{keys.filter(f.test).length}</span>
              </button>
            ))}
          </div>
        </>
      )}
      {shown.length === 0 && <p className="help no-match">No keys match{q ? ` \u201c${query.trim()}\u201d` : ""}.</p>}
      <div className="keys">
        {shown.map((k) => (
          <div className="key-row" key={k.id} title={describe(k)}>
            <span className="key-ico"><Icon name="key" /></span>
            <span className="who">
              <span className="nm">{k.label}</span>
              <span className="meta">{k.from ? `From ${k.from}` : "Yours"} · {lifetime(k)}</span>
            </span>
            <button className="use" onClick={() => void run(api.copyPointer(k.id))} title="Copy a line for your .env file. It points at the key, it doesn't hold it.">
              Copy .env line
            </button>
            <button
              className={`forget ${armed === k.id ? "sure" : ""}`}
              aria-label={`Forget ${k.label}`}
              onClick={() => {
                if (armed === k.id) void run(api.forget(k.id));
                else setArmed(k.id);
              }}
              onBlur={() => setArmed(null)}
            >
              {armed === k.id ? "Forget for good?" : <Icon name="close" />}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

function Me({ name, pending, run }: { name: string; pending: number; run: Run }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(name);
  const save = () => {
    setEditing(false);
    if (draft.trim() && draft !== name) void run(api.setName(draft));
  };
  return (
    <div className="foot">
      <span className="me">
        {editing ? (
          <input
            autoFocus
            value={draft}
            maxLength={64}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={save}
            onKeyDown={(e) => e.key === "Enter" && save()}
            aria-label="Your name"
          />
        ) : (
          <button onClick={() => { setDraft(name); setEditing(true); }} title="Shown on your requests">
            You are <b>{name}</b>
          </button>
        )}
        {pending > 0 && <span> · {pending} waiting</span>}
      </span>
      <button className="paste" onClick={() => void run(api.paste())} title="Paste a .handoff file">
        Paste a file <kbd>Ctrl V</kbd>
      </button>
    </div>
  );
}

const KEEP_HELP: Record<Keep, string> = {
  once: "Opens one time, then it's destroyed.",
  days30: "Destroyed automatically in 30 days.",
  keep: "Kept until you remove it.",
};

/** Opening a sealed key: it unlocks itself, then one tap picks how long to keep it. */
function AcceptSheet({ item, run }: { item: KeyView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [item.id]);
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Tile name="sealed" size="lg" />
        <div>
          <span className="lede">{item.from ? `${item.from} sent you a key` : "Your key"}</span>
          <h2 className="display md">{item.label}</h2>
        </div>
      </div>
      <div className="keep-block">
        <span className="label">Keep it for</span>
        <div className="segmented">
          {KEEP_CHOICES.map((c) => (
            <button
              key={c.keep}
              className={item.keep === c.keep ? "pick" : ""}
              aria-pressed={item.keep === c.keep}
              onClick={() => void run(api.setKeep(item.id, c.keep).then(api.dismiss))}
            >
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

/** A program asks for kept keys through a `handoff:` pointer. */
function ProgramSheet({ program, run }: { program: ProgramView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [program.path, program.folder]);
  const one = program.keys.length === 1;
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Tile name="key" size="lg" />
        <div>
          <span className="lede">A program wants {one ? "a key" : `${program.keys.length} keys`}</span>
          <h2 className="display md">{program.name}</h2>
        </div>
      </div>
      <div className="chip">
        <Icon name="file" />
        <span className="mono">{program.command ?? program.name}</span>
      </div>
      <p className="lede">
        It asks for{" "}
        {program.keys.map((k, i) => (
          <span key={k}>{i > 0 && ", "}<strong>{k}</strong></span>
        ))}{" "}
        from <span className="mono">{program.folder ?? "a folder KeyHandoff can't see"}</span>.
      </p>
      <p className="help">
        <span className="mono">{program.path}</span>. Always allow covers this program in this folder; allow only programs you
        started.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void run(api.answer("deny"))}>Deny</button>
        <button className="btn secondary" onClick={() => void run(api.answer("once"))}>Just this once</button>
        <button className="btn primary grow" autoFocus onClick={() => void run(api.answer("always"))}>Always allow</button>
      </div>
    </div>
  );
}

/** After Ask: the request file is on the clipboard; it can also be dragged out. */
function AskSheet({ fileName, computer, run }: { fileName: string; computer: string; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  const drag = useDragOut(() => void run(api.dragOut("request")));
  useEffect(() => void enter(sheet.current, "rise", 360), [fileName]);
  return (
    <div className="sheet" ref={sheet}>
      <Tile name="send" size="lg" />
      <div className="hero">
        <h2 className="display md">Request copied.</h2>
        <p className="lede">Paste it into a chat with the person who has the key. It holds no secret, so any chat is fine.</p>
      </div>
      <div className="chip grab" {...drag}>
        <Icon name="file" />
        <span className="mono">{fileName}</span>
      </div>
      <p className="help">
        Whoever opens it sees your computer as <span className="mono">{computer}</span>. Read it to them if they want to
        check it's really you. When the sealed reply comes back, paste it here.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void run(api.copyRequest())}>Copy again</button>
        <button className="btn primary grow" onClick={() => void api.dismiss()}>Done</button>
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

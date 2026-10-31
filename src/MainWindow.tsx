import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Glyph, Icon, Logo, useDragOut } from "./art";
import { api, attempt, KEEP_CHOICES, useView, type ContactView, type Keep, type KeyView, type ProgramView, type View } from "./ipc";
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
  const bad = error ?? view?.problem ?? null;
  const toast = view?.program ? null : (bad ?? view?.notice ?? null);
  const safeOn = view !== null && view.safe.state !== "off";
  const [sending, setSending] = useState(false);

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
        {view && !safeOn && <Home view={view} run={run} onSend={() => setSending(true)} />}
      </main>
      {view && safeOn && <SafeCopy view={view} />}
      {view?.accepted && <AcceptSheet item={view.accepted} run={run} />}
      {view?.asked && !view.accepted && <CardSheet fileName={view.asked} computer={view.computer} run={run} />}
      {view?.added && <ContactSheet contact={view.added} run={run} />}
      {view && sending && !safeOn && (
        <SendSheet contacts={view.contacts} run={run} onClose={() => setSending(false)} />
      )}
      {view?.program && <ProgramSheet program={view.program} run={run} />}
      {toast && (
        <button className={`toast ${bad ? "bad" : ""}`} onClick={() => { setError(null); void api.dismiss(); }}>
          {!bad && <Icon name="check" />}
          {toast}
        </button>
      )}
    </div>
  );
}

type Run = (p: Promise<unknown>) => Promise<void>;

function Home({ view, run, onSend }: { view: View; run: Run; onSend: () => void }) {
  const n = view.keys.length;
  return (
    <>
      <div className="hero">
        <h1 className="display sm">{n === 0 ? "No keys yet." : "Your keys"}</h1>
        <p className="lede">{n === 0 ? "Add a key, or send one to someone." : summary(view.keys)}</p>
      </div>
      <div className="btn-row">
        <button className="btn primary grow" onClick={() => void run(api.add())}>Add a key</button>
        <button className="btn secondary grow" onClick={onSend}>Send a key</button>
      </div>
      <Vault keys={view.keys} run={run} />
      <Me name={view.name} run={run} />
    </>
  );
}

/** "3 keys · 1 expires this week": the count, and the one thing that needs you, if any. */
function summary(keys: KeyView[]): string {
  const now = Date.now() / 1000;
  const soon = keys.filter((k) => k.expires_at !== null && k.expires_at - now < 7 * 24 * 60 * 60).length;
  const once = keys.filter((k) => k.keep === "once").length;
  const parts = [`${keys.length} ${keys.length === 1 ? "key" : "keys"}`];
  if (soon > 0) parts.push(`${soon} ${soon === 1 ? "expires" : "expire"} this week`);
  else if (once > 0) parts.push(`${once} ${once === 1 ? "opens" : "open"} once`);
  else parts.push("all sealed on this computer");
  return parts.join(" · ");
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
        <li><span className="n">1</span><span><b>Add a key</b>: copy it from any page, and it's sealed here. It's never shown.</span></li>
        <li><span className="n">2</span><span><b>Get keys from people</b>: send them <b>Your card</b>, at the bottom. They only need it once.</span></li>
        <li><span className="n">3</span><span><b>Send a key</b>: open someone's card once, then send them keys any time.</span></li>
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

function Me({ name, run }: { name: string; run: Run }) {
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
          <button onClick={() => { setDraft(name); setEditing(true); }} title="Shown on your card">
            You are <b>{name}</b>
          </button>
        )}
        <span> · </span>
        <button onClick={() => void run(api.copyCard())} title="Copy your card to send to people who'll send you keys">
          <b>Your card</b>
        </button>
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
        <Glyph mark="check" />
        <div>
          <span className="lede">{item.from ? `${item.from} sent you a key` : "Your key"}</span>
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

/** A program asks for kept keys through a `handoff:` pointer. */
function ProgramSheet({ program, run }: { program: ProgramView; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [program.path, program.folder]);
  const one = program.keys.length === 1;
  return (
    <div className="sheet" ref={sheet}>
      <div className="head">
        <Glyph base="window" mark="key" />
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

/** Your card is on the clipboard; it can also be dragged out. */
function CardSheet({ fileName, computer, run }: { fileName: string; computer: string; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  const drag = useDragOut(() => void run(api.dragOut("request")));
  useEffect(() => void enter(sheet.current, "rise", 360), [fileName]);
  return (
    <div className="sheet" ref={sheet}>
      <Glyph mark="send" />
      <div className="hero">
        <h2 className="display md">Card copied.</h2>
        <p className="lede">Send it to anyone who'll send you keys, by chat or email. It holds no secret, and they only need it once.</p>
      </div>
      <div className="chip grab" {...drag}>
        <Icon name="file" />
        <span className="mono">{fileName}</span>
      </div>
      <p className="help">
        It's good for 12 keys. They'll see your computer as <span className="mono">{computer}</span>; read it to them if they
        want to check it's really you.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void run(api.copyRequest())}>Copy again</button>
        <button className="btn primary grow" onClick={() => void api.dismiss()}>Done</button>
      </div>
    </div>
  );
}

/** Someone's card was just opened: they're a contact now. */
function ContactSheet({ contact, run }: { contact: ContactView; run: Run }) {
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
        <button className="btn primary grow" onClick={() => void run(api.dismiss().then(() => api.sendTo(contact.id)))}>
          Send {contact.name} a key
        </button>
      </div>
    </div>
  );
}

/** Send a key: pick who it's for, then Safe Copy catches it for them. */
function SendSheet({ contacts, run, onClose }: { contacts: ContactView[]; run: Run; onClose: () => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), []);
  return (
    <div className="sheet" ref={sheet}>
      <div className="hero">
        <h2 className="display md">Send a key</h2>
        <p className="lede">{contacts.length ? "Who's it for?" : "Nobody to send to yet."}</p>
      </div>
      {contacts.length ? (
        <div className="keys">
          {contacts.map((c) => (
            <button key={c.id} className="key-row contact" onClick={() => void run(api.sendTo(c.id)).then(onClose)}>
              <span className="key-ico"><Icon name="user" /></span>
              <span className="who">
                <span className="nm">{c.name}</span>
                <span className="meta">
                  {c.sends_left > 0 ? `${c.sends_left} ${c.sends_left === 1 ? "send" : "sends"} left` : "Card used up: ask for a new one"}
                  {" · "}
                  <span className="mono">{c.computer}</span>
                </span>
              </span>
            </button>
          ))}
        </div>
      ) : (
        <p className="help">
          Ask the person for their KeyHandoff card, then open it here: double-click it, drop it on this window, or paste it with
          Ctrl V.
        </p>
      )}
      <span className="grow-space" />
      <button className="btn secondary" onClick={onClose}>Close</button>
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

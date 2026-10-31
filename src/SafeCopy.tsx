import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Icon, useDragOut } from "./art";
import { api, attempt, KEEP_CHOICES, type KeyView, type SafeState, type SafeView, type View } from "./ipc";
import { enter } from "./motion";

const STATUS: Record<SafeState, string> = {
  off: "Off", listening: "Listening", timeout: "Off", failed: "Off", caught: "Caught", sealed: "Sealed",
};

/**
 * Safe Copy, shown in place of the home screen while it is on; Rust keeps the
 * window on top meanwhile. Rust does the listening, wiping and sealing; this
 * only shows the state and never sees the key.
 */
export function SafeCopy({ view }: { view: View }) {
  const body = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const state = view.safe.state;

  // Each state builds in fresh: the sealed file zooms in, the others rise.
  useLayoutEffect(() => {
    setError(null);
    void enter(body.current, state === "sealed" ? "zoom" : "rise", state === "sealed" ? 420 : 300);
  }, [state]);

  const run = async (p: Promise<unknown>) => setError(await attempt(p));
  const safe = view.safe;
  const who = safe.for_name ? `For ${safe.for_name}` : "For you";

  return (
    <div className={`safe ${state}`}>
      <div className="top">
        <i className="dot" />
        <b>Safe Copy</b>
        <span>{STATUS[state]} · {who}</span>
      </div>
      <div key={state} ref={body} className="safe-body">
        {safe.for_computer && state === "listening" && (
          <div className="asking">
            <span>{safe.for_name} is asking</span>
            <span className="mono">computer {safe.for_computer}</span>
          </div>
        )}
        {state === "listening" && <Listening safe={safe} />}
        {(state === "timeout" || state === "failed") && (
          <>
            <h2 className="title">{state === "timeout" ? "Time ran out" : "Safe Copy stopped"}</h2>
            <p className="help">{safe.message ?? "Nothing was caught. The clipboard was not read."}</p>
            <span className="grow-space" />
            <div className="btn-row">
              <button className="btn secondary small" onClick={() => void api.closeSafe()}>Cancel</button>
              <button className="btn primary small grow" onClick={() => void run(api.retry())}>Try again</button>
            </div>
          </>
        )}
        {state === "caught" && <Caught view={view} run={run} />}
        {state === "sealed" && <Sealed safe={safe} run={run} />}
        {error && <p className="err">{error}</p>}
      </div>
    </div>
  );
}

type Run = (p: Promise<unknown>) => Promise<void>;

const LISTEN_MS = 60000;
const COPY_MOD = /Mac|iPhone|iPad/.test(navigator.platform) ? "\u2318" : "Ctrl";

/** Listening: the copy shortcut pressing inside a countdown ring says what to do. */
function Listening({ safe }: { safe: SafeView }) {
  const [left, setLeft] = useState(() => Math.max(0, safe.ends_at - Date.now()));
  const [info, setInfo] = useState(false);
  useEffect(() => {
    const t = window.setInterval(() => setLeft(Math.max(0, safe.ends_at - Date.now())), 250);
    return () => window.clearInterval(t);
  }, [safe.ends_at]);
  const r = 82;
  const c = 2 * Math.PI * r;
  const fraction = Math.min(1, left / LISTEN_MS);
  return (
    <>
      <div className="copy-now" aria-hidden>
        <svg className="ring" viewBox="0 0 172 172">
          <circle className="track" cx="86" cy="86" r={r} />
          <circle className="arc" cx="86" cy="86" r={r} strokeDasharray={c} strokeDashoffset={c * (1 - fraction)} />
        </svg>
        <span className="ripple" />
        <span className="caps">
          <kbd className="cap mod">{COPY_MOD}</kbd>
          <kbd className="cap c">C</kbd>
        </span>
      </div>
      <div className="hero-text center">
        <h2 className="title">Copy your key</h2>
        <p className="help">From anywhere · {Math.ceil(left / 1000)}s</p>
      </div>
      {safe.apps.length > 0 && (
        <p className="apps">
          {andList(safe.apps)} {safe.apps.length === 1 ? "is" : "are"} running. Clipboard apps can read the key when you copy it.
        </p>
      )}
      {info && (
        <p className="info">
          KeyHandoff catches that one copy, wipes your clipboard and removes it from clipboard history where the
          system allows, then you name the key. It's never shown. A program watching your clipboard can still read
          it as you copy.
        </p>
      )}
      <span className="grow-space" />
      <div className="btn-row">
        <button className="link" onClick={() => setInfo(!info)} aria-expanded={info}>
          {info ? "Hide" : "How it works"}
        </button>
        <span className="grow-space" />
        <button className="btn secondary small" onClick={() => void api.closeSafe()}>Cancel</button>
      </div>
    </>
  );
}

/** "Ditto", "Ditto and CopyQ", "Ditto, CopyQ and ClipX". */
function andList(names: string[]): string {
  return names.length < 2 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** Caught and wiped. Name it, then seal it (for someone) or pick a keep-time (for you). */
function Caught({ view, run }: { view: View; run: Run }) {
  const forSomeone = view.safe.for_name !== null;
  // A key for you lands in your own vault, where names are unique.
  const [label, setLabel] = useState(() => (forSomeone ? "API key" : freeName("API key", view.keys)));
  const input = useRef<HTMLInputElement>(null);
  const savedAs = forSomeone ? label : freeName(label, view.keys);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  return (
    <>
      <div className="caught-head">
        <Icon name="check" />
        <h2 className="title">Caught. Clipboard wiped.</h2>
      </div>
      {view.safe.message && <p className="help">{view.safe.message}</p>}
      <label className="field">
        Name this key
        <input
          ref={input}
          className="name-in"
          value={label}
          maxLength={64}
          placeholder="Name this key"
          onChange={(e) => setLabel(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            void run(forSomeone ? api.seal(label) : api.keepOwn(label, view.keep));
          }}
        />
      </label>
      {savedAs !== label.trim() && <p className="help">That name is taken, so it's saved as {savedAs}.</p>}
      {forSomeone ? (
        <button className="btn primary" onClick={() => void run(api.seal(label))}>
          Seal for {view.safe.for_name}
        </button>
      ) : (
        <div className="keep-block">
          <span className="label">Keep it for</span>
          <div className="keep-buttons">
            {KEEP_CHOICES.map((c) => (
              <button
                key={c.keep}
                className={view.keep === c.keep ? "pick" : ""}
                onClick={() => void run(api.keepOwn(label, c.keep))}
              >
                {c.label}
              </button>
            ))}
          </div>
          <p className="help">One tap saves it.</p>
        </div>
      )}
      <span className="grow-space" />
      <button className="btn secondary small" onClick={() => void api.closeSafe()}>Cancel</button>
    </>
  );
}

/** Mirrors Vault::free_name: `label`, or `label 2`, `label 3`… when taken. */
function freeName(label: string, keys: KeyView[]): string {
  const base = label.trim();
  const taken = (name: string) => keys.some((k) => k.label.trim().toLowerCase() === name.toLowerCase());
  if (!taken(base)) return base;
  let n = 2;
  while (taken(`${base} ${n}`)) n++;
  return `${base} ${n}`;
}

function Sealed({ safe, run }: { safe: SafeView; run: Run }) {
  const [copied, setCopied] = useState(false);
  const drag = useDragOut(() => void run(api.dragOut("sealed")));
  return (
    <>
      <div className="hero">
        <h2 className="title">Sealed for {safe.for_name}.</h2>
        <p className="help">Only {safe.for_name}'s computer can open it. This computer keeps nothing.</p>
      </div>
      <div className="chip grab" {...drag}>
        <Icon name="file" />
        <span className="mono">{safe.file_name}</span>
      </div>
      <button className="btn primary" onClick={() => void run(api.copySealed().then(() => setCopied(true)))}>
        {copied ? "Copied" : "Copy sealed file"}
      </button>
      <p className="help">{copied ? `Paste it into your chat with ${safe.for_name}.` : `Then paste it into your chat with ${safe.for_name}.`}</p>
      <span className="grow-space" />
      <button className="btn secondary small" onClick={() => void api.closeSafe()}>Done</button>
    </>
  );
}

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Icon, Pages, useDragOut } from "./art";
import { api, attempt, KEEP_CHOICES, type KeyView, type SafeView, type View } from "./ipc";
import { enter } from "./motion";

/**
 * Safe Copy, shown in place of the home screen while it is on; Rust keeps the
 * window on top meanwhile. Rust does the listening, wiping and sealing; this
 * only shows the state and never sees the key.
 *
 * Listening and caught share one stage, the pages and the headline, which stays
 * put while the key lands: only the check draws on and the words change. What
 * differs between the two sits below the stage.
 */
export function SafeCopy({ view }: { view: View }) {
  const body = useRef<HTMLDivElement>(null);
  const below = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState(false);
  const state = view.safe.state;
  const staged = state === "listening" || state === "caught";

  // The stage never re-enters; the part below it fades in. The sealed file
  // zooms in, and the remaining states rise.
  useLayoutEffect(() => {
    setError(null);
    if (staged) void enter(below.current, "fade", 220);
    else void enter(body.current, state === "sealed" ? "zoom" : "rise", state === "sealed" ? 420 : 300);
  }, [state, staged]);

  const run = async (p: Promise<unknown>) => setError(await attempt(p));
  const safe = view.safe;

  return (
    <div className={`safe ${state}`}>
      <div className="top">
        <Icon name="shield" />
        <b>Safe Copy</b>
        {safe.for_name && <span>for {safe.for_name}</span>}
      </div>
      {staged ? (
        <div className="safe-body">
          <div className="stage">
            <Pages caught={state === "caught"} />
            <h2 className="display">{state === "caught" ? "Caught" : "Copy your key"}</h2>
            <p className="sub">{state === "caught" ? "Clipboard wiped." : <Countdown endsAt={safe.ends_at} />}</p>
          </div>
          <div key={state} ref={below} className="below">
            {state === "listening" ? <Listening safe={safe} info={info} /> : <Caught view={view} run={run} />}
            {error && <p className="err">{error}</p>}
          </div>
          {/* Outside the part that swaps, so Cancel never moves or changes. */}
          <div className="btn-row safe-foot">
            {state === "listening" && (
              <button className="link" onClick={() => setInfo(!info)} aria-expanded={info}>
                {info ? "Hide" : "How it works"}
              </button>
            )}
            <span className="grow-space" />
            <button className="btn secondary small" onClick={() => void api.closeSafe()}>Cancel</button>
          </div>
        </div>
      ) : (
        <div key={state} ref={body} className="safe-body">
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
          {state === "sealed" && <Sealed safe={safe} run={run} />}
          {error && <p className="err">{error}</p>}
        </div>
      )}
    </div>
  );
}

type Run = (p: Promise<unknown>) => Promise<void>;

/** "From anywhere · 59s", counting down to when listening stops. */
function Countdown({ endsAt }: { endsAt: number }) {
  const [left, setLeft] = useState(() => Math.max(0, endsAt - Date.now()));
  useEffect(() => {
    const t = window.setInterval(() => setLeft(Math.max(0, endsAt - Date.now())), 250);
    return () => window.clearInterval(t);
  }, [endsAt]);
  return <>From anywhere · {Math.ceil(left / 1000)}s</>;
}

/** Below the stage while listening: who is asking, clipboard apps and the info. */
function Listening({ safe, info }: { safe: SafeView; info: boolean }) {
  return (
    <>
      {safe.for_computer && (
        <div className="asking">
          <span>{safe.for_name} is asking</span>
          <span className="mono">computer {safe.for_computer}</span>
        </div>
      )}
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
  const save = (keep = view.keep) => void run(forSomeone ? api.seal(label) : api.keepOwn(label, keep));
  return (
    <>
      {view.safe.message && <p className="help center">{view.safe.message}</p>}
      <input
        ref={input}
        className="name-in"
        value={label}
        maxLength={64}
        placeholder="Name this key"
        aria-label="Name this key"
        onChange={(e) => setLabel(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && save()}
      />
      {savedAs !== label.trim() && <p className="help">That name is taken, so it's saved as {savedAs}.</p>}
      {forSomeone ? (
        <button className="btn primary" onClick={() => save()}>
          Seal for {view.safe.for_name}
        </button>
      ) : (
        <div className="keep-buttons" role="group" aria-label="How long to keep it">
          {KEEP_CHOICES.map((c) => (
            <button key={c.keep} className={view.keep === c.keep ? "pick" : ""} onClick={() => save(c.keep)}>
              <Icon name={c.icon} />
              {c.label}
            </button>
          ))}
        </div>
      )}
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

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
  const below = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState(false);
  const state = view.safe.state;
  const safe = view.safe;

  // The stage never re-enters; only the part below it fades in.
  useLayoutEffect(() => {
    setError(null);
    void enter(below.current, "fade", 220);
  }, [state]);

  const run = async (p: Promise<unknown>) => setError(await attempt(p));
  const missed = state === "timeout" || state === "failed";
  const stage = {
    listening: { mark: "none", title: "Copy your key", sub: <Countdown endsAt={safe.ends_at} /> },
    caught: { mark: "check", title: "Caught", sub: "Clipboard wiped." },
    sealed: { mark: "lock", title: "Sealed", sub: `Only ${safe.for_name}'s computer can open it.` },
    timeout: { mark: "missed", title: "Time ran out", sub: "Nothing was read." },
    failed: { mark: "missed", title: "Stopped", sub: "Nothing was caught." },
    off: { mark: "none", title: "", sub: "" },
  } as const;
  const { mark, title, sub } = stage[state];

  return (
    <div className={`safe ${state}`}>
      <div className="top">
        <Icon name="shield" />
        <b>Safe Copy</b>
        {safe.for_name && <span>for {safe.for_name}</span>}
      </div>
      <div className="safe-body">
        <div className="safe-stage">
          <Pages mark={mark} />
          <h2 className="display">{title}</h2>
          <p className="sub">{sub}</p>
        </div>
        <div key={state} ref={below} className="below">
          {state === "listening" && <Listening safe={safe} info={info} />}
          {state === "caught" && <Caught view={view} run={run} />}
          {state === "sealed" && <Sealed safe={safe} run={run} />}
          {missed && (
            <>
              {state === "failed" && safe.message && <p className="help center">{safe.message}</p>}
              <button className="btn primary" onClick={() => void run(api.retry())}>Try again</button>
            </>
          )}
          {error && <p className="err">{error}</p>}
        </div>
        {/* Outside the part that swaps, so this button never moves. */}
        <div className="btn-row safe-foot">
          {state === "listening" && (
            <button className="link" onClick={() => setInfo(!info)} aria-expanded={info}>
              {info ? "Hide" : "How it works"}
            </button>
          )}
          <span className="grow-space" />
          <button className="btn secondary small" onClick={() => void api.closeSafe()}>{state === "sealed" ? "Done" : "Cancel"}</button>
        </div>
      </div>
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
      <ClipboardWarning safe={safe} />
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

/**
 * One message about what might read the key as you copy it: whether
 * something read the bait KeyHandoff put on the clipboard, together with any
 * known clipboard apps running (from the process list, never from the read).
 */
function ClipboardWarning({ safe }: { safe: SafeView }) {
  const apps = safe.apps.length > 0 ? `${andList(safe.apps)} ${safe.apps.length === 1 ? "is" : "are"} running` : null;
  if (safe.read) {
    return (
      <div className="warning" role="alert">
        <Icon name="alert" />
        <div>
          <b>Something just read your clipboard.</b>
          <p>
            {apps
              ? `${apps} and may be what read it. Close ${safe.apps.length === 1 ? "it" : "them"} before you copy, or the key could end up in its history.`
              : "KeyHandoff can't tell what. Before you copy, close programs that watch the clipboard, or check your antivirus."}
          </p>
        </div>
      </div>
    );
  }
  return apps ? <p className="apps">{apps}. Clipboard apps can read the key when you copy it.</p> : null;
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
      <div className="file-row">
        <div className="chip grab" {...drag}>
          <Icon name="file" />
          <span className="mono">{safe.file_name}</span>
        </div>
        <button className="btn primary" onClick={() => void run(api.copySealed().then(() => setCopied(true)))}>
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <p className="help center">{copied ? "Now paste it into your chat." : "This computer keeps nothing."}</p>
    </>
  );
}

import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { typingApi, type TypingView } from "./ipc";
import { Icon, Logo } from "./art";

/** This native window does not take focus: clicking Type leaves the field selected. */
export function TypingPanel() {
  const [view, setView] = useState<TypingView | null>(null);
  const [now, setNow] = useState(Date.now());
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const refresh = () => void typingApi.view().then((v) => {
      if (live) { setView(v); setNow(Date.now()); }
    }).catch((e) => { if (live) setError(String(e)); });
    const off = listen("typing-changed", refresh);
    void off.then(refresh);
    return () => { live = false; void off.then((stop) => stop()); };
  }, []);
  useEffect(() => {
    setError(null);
    if (!view?.key) return;
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(timer);
  }, [view?.token, view?.key?.id]);
  const seconds = Math.max(0, Math.ceil(((view?.ends_at ?? 0) - now) / 1000));
  const armed = Boolean(view?.key && seconds > 0);
  const busy = sending || view?.busy;
  const send = async () => {
    if (!view || !armed || busy) return;
    setSending(true);
    setError(null);
    try { await typingApi.send(view.token, view.revision); } catch (e) { setError(String(e)); }
    finally { setSending(false); }
  };
  return (
    <div className="typing-panel">
      <header className="titlebar" data-tauri-drag-region>
        <Logo /><span className="name" data-tauri-drag-region>Type a saved key</span>
        <button aria-label="Close typing panel" onClick={() => void typingApi.cancel()}><Icon name="close" /></button>
      </header>
      <main className="typing-body">
        {busy ? <><h1 className="title">Typing…</h1><p className="lede">Keep the destination field selected.</p></> : armed ? <>
          <div className="typing-heading"><h1 className="title">{view!.key!.label}</h1><span className="typing-clock" aria-hidden="true">Armed · {seconds}s</span></div>
          <p className="lede">Click the field you want to fill, then click Type here.</p>
          <p className="typing-warning">This field may show your key. Check the destination before typing.</p>
          <button className="btn primary typing-confirm" disabled={!view?.target} title={view?.target ?? undefined} onClick={() => void send()}>
            {view?.target ? `Type into ${view.target}` : "Click a destination field first"}
          </button>
          {error && <p className="err" role="alert">{error}</p>}
          <p className="help">{view?.message ?? "Uses keystrokes, without the clipboard. Keyboard watchers can still read them."}</p>
          {view!.key!.keep === "once" && <p className="help">Use once: deleted as typing begins, even if it stops partway.</p>}
        </> : <>
          <h1 className="title">{view?.message?.startsWith("Typed ") ? "Typed." : "Disarmed."}</h1>
          <p className="lede" role="status">{error ?? view?.message ?? "Choose Type on a saved key to start again."}</p>
          <button className="btn secondary" onClick={() => void typingApi.cancel()}>Done</button>
        </>}
      </main>
    </div>
  );
}

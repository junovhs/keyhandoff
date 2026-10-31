import { useState } from "react";
import { Pages } from "./art";
import { attempt, gateApi, type GateView } from "./ipc";

/** Before the vault opens: choose a passphrase, unlock, or recover. Laid out
 * like Home (headline, then the form, buttons at the bottom), so opening the
 * vault swaps the words and keeps the window still. */
export function Gate({ gate }: { gate: GateView }) {
  if (gate.recovery) return <RecoveryCode code={gate.recovery} />;
  if (gate.state === "setup") return <SetUp gate={gate} />;
  return <Unlock gate={gate} />;
}

function Art() {
  return (
    <div className="gate-art">
      <Pages mark="lock" />
    </div>
  );
}

/** A passphrase box with Show. */
function Passphrase({ value, onChange, label, autoFocus, shown, onShown }: {
  value: string;
  onChange: (v: string) => void;
  label: string;
  autoFocus?: boolean;
  shown: boolean;
  onShown: (s: boolean) => void;
}) {
  return (
    <div className="pass">
      <input
        className="name-in"
        type={shown ? "text" : "password"}
        value={value}
        autoFocus={autoFocus}
        spellCheck={false}
        autoComplete="off"
        placeholder={label}
        aria-label={label}
        onChange={(e) => onChange(e.target.value)}
      />
      <button type="button" className="quiet" onClick={() => onShown(!shown)}>{shown ? "Hide" : "Show"}</button>
    </div>
  );
}

/** Run an unlock-type call, with a busy state while Argon2id works. */
function useBusy() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (p: () => Promise<unknown>) => {
    setBusy(true);
    setError(await attempt(p()));
    setBusy(false);
  };
  return { busy, error, run };
}

function SetUp({ gate }: { gate: GateView }) {
  const [pass, setPass] = useState("");
  const [again, setAgain] = useState("");
  const [shown, setShown] = useState(false);
  // Suggested words show whole, wrapping, so none is cut off by the box.
  const [suggested, setSuggested] = useState<string | null>(null);
  const { busy, error, run } = useBusy();
  const short = pass.trim().length < gate.min_passphrase;
  const differs = !shown && !suggested && again !== pass;
  const suggest = async () => {
    const words = await gateApi.suggest();
    setSuggested(words);
    setPass(words);
  };
  const own = () => {
    setSuggested(null);
    setPass("");
    setAgain("");
  };
  return (
    <form className="gate" onSubmit={(e) => { e.preventDefault(); if (!short && !differs) void run(() => gateApi.setUp(pass)); }}>
      <Art />
      <div className="hero">
        <h1 className="display sm">{gate.migrating ? "Lock your keys" : "Choose a passphrase"}</h1>
        <p className="lede">
          {gate.migrating
            ? "KeyHandoff now locks your keys with a passphrase. While it's locked, no other program can open them."
            : "While KeyHandoff is locked, no other program on this computer can open your keys."}
        </p>
      </div>
      {suggested ? (
        <>
          <p className="code words">{suggested}</p>
          <p className="help">
            Learn these five words, or write them down somewhere safe.{" "}
            <button type="button" className="link inline" onClick={() => void suggest()}>Other words</button>
            {" · "}
            <button type="button" className="link inline" onClick={own}>Type my own</button>
          </p>
        </>
      ) : (
        <>
          <Passphrase value={pass} onChange={setPass} label="Passphrase" autoFocus shown={shown} onShown={setShown} />
          {!shown && <Passphrase value={again} onChange={setAgain} label="Type it again" shown={false} onShown={setShown} />}
          <p className="help">
            Five random words is best. <button type="button" className="link inline" onClick={() => void suggest()}>Suggest five</button>
          </p>
        </>
      )}
      {error && <p className="err">{error}</p>}
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn primary grow" disabled={busy || short || differs}>
          {busy ? "Locking…" : short ? `At least ${gate.min_passphrase} characters` : differs && again ? "They don't match" : "Set passphrase"}
        </button>
      </div>
    </form>
  );
}

type Mode = "pin" | "pass" | "recover";

function Unlock({ gate }: { gate: GateView }) {
  const [mode, setMode] = useState<Mode>(gate.pin ? "pin" : "pass");
  const [pass, setPass] = useState("");
  const [code, setCode] = useState("");
  const [shown, setShown] = useState(false);
  const { busy, error, run } = useBusy();
  const short = pass.trim().length < (mode === "recover" ? gate.min_passphrase : 1);
  const go = (m: Mode) => {
    setMode(m);
    setPass("");
  };
  const submit = () => {
    if (short) return;
    void run(() =>
      mode === "pin" ? gateApi.unlockPin(pass) : mode === "recover" ? gateApi.recover(code, pass) : gateApi.unlock(pass),
    );
  };
  const words = {
    pin: ["KeyHandoff is locked", "Type your PIN to open your keys.", "PIN"],
    pass: ["KeyHandoff is locked", "Type your passphrase to open your keys.", "Passphrase"],
    recover: ["Use your recovery code", "Type the code you wrote down, then choose a new passphrase.", "New passphrase"],
  }[mode];
  return (
    <form className="gate" onSubmit={(e) => { e.preventDefault(); submit(); }}>
      <Art />
      <div className="hero">
        <h1 className="display sm">{words[0]}</h1>
        <p className="lede">{words[1]}</p>
      </div>
      {mode === "recover" && (
        <input
          className="name-in mono"
          value={code}
          autoFocus
          spellCheck={false}
          autoComplete="off"
          placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
          aria-label="Recovery code"
          onChange={(e) => setCode(e.target.value)}
        />
      )}
      <Passphrase
        key={mode}
        value={pass}
        onChange={setPass}
        label={words[2]}
        autoFocus={mode !== "recover"}
        shown={shown}
        onShown={setShown}
      />
      {error && <p className="err">{error}</p>}
      <span className="grow-space" />
      <p className="help links">
        {mode !== "pass" && <button type="button" className="link inline" onClick={() => go("pass")}>Use your passphrase</button>}
        {mode === "pass" && gate.pin && <button type="button" className="link inline" onClick={() => go("pin")}>Use your PIN</button>}
        {mode !== "recover" && <button type="button" className="link inline" onClick={() => go("recover")}>Forgot it? Use your recovery code</button>}
      </p>
      <div className="btn-row">
        <button className="btn primary grow" disabled={busy || short}>
          {busy ? "Opening…" : mode === "recover" ? "Open and set passphrase" : "Unlock"}
        </button>
      </div>
    </form>
  );
}

function RecoveryCode({ code }: { code: string }) {
  const [sure, setSure] = useState(false);
  return (
    <div className="gate">
      <Art />
      <div className="hero">
        <h1 className="display sm">Write down this code</h1>
        <p className="lede">If you forget your passphrase, this code is the only way back to your keys.</p>
      </div>
      <p className="code mono">{code}</p>
      <p className="help">
        Keep it on paper, not on this computer. It's shown this once; you can't see it again.
      </p>
      <span className="grow-space" />
      <div className="btn-row">
        {sure ? (
          <button className="btn primary grow" onClick={() => void gateApi.recoveryDone()}>Yes, it's written down</button>
        ) : (
          <button className="btn primary grow" onClick={() => setSure(true)}>I've written it down</button>
        )}
      </div>
    </div>
  );
}

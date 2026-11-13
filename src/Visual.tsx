// Picture-first screens: the empty home, importing a .env, and using a key in
// code. Each says what happens with icons and a few words; the help text is
// one line at most. Icons are Lucide, the same set as keyhandoff.dev.
import { useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  Check,
  ClipboardCopy,
  Copy,
  FileCode2,
  FileInput,
  IdCard,
  KeyRound,
  Link2,
  ShieldCheck,
  SquareTerminal,
  Vault,
} from "lucide-react";
import { Icon } from "./art";
import { api, KEEP_CHOICES, type ImportView, type Keep, type View } from "./ipc";
import { enter } from "./motion";

type Run = (p: Promise<unknown>) => Promise<void>;

const ICON = { size: 22, strokeWidth: 1.75, "aria-hidden": true } as const;

/** No keys yet: three ways in, as tiles. */
export function StartTiles({ run }: { run: Run }) {
  const tiles = [
    { icon: <ClipboardCopy {...ICON} />, title: "Copy a key", sub: "from any page", act: () => void run(api.add()) },
    { icon: <FileInput {...ICON} />, title: "Import a .env", sub: "keys out of a file", act: () => void run(api.importEnv()) },
    { icon: <IdCard {...ICON} />, title: "Get keys", sub: "send your card", act: () => void run(api.copyCard()) },
  ];
  return (
    <div className="tiles">
      {tiles.map((t) => (
        <button key={t.title} className="tile" onClick={t.act}>
          <span className="tile-ico">{t.icon}</span>
          <span className="tile-title">{t.title}</span>
          <span className="tile-sub">{t.sub}</span>
        </button>
      ))}
    </div>
  );
}

/** A picture of where things go: from → to. */
function Flow({ from, fromLabel, to, toLabel }: { from: JSX.Element; fromLabel: string; to: JSX.Element; toLabel: string }) {
  return (
    <div className="flow" aria-hidden>
      <span className="flow-end">
        <span className="flow-ico">{from}</span>
        <span className="flow-label mono">{fromLabel}</span>
      </span>
      <ArrowRight size={20} strokeWidth={1.75} className="flow-arrow" />
      <span className="flow-end on">
        <span className="flow-ico">{to}</span>
        <span className="flow-label">{toLabel}</span>
      </span>
    </div>
  );
}

/** Importing a .env: tick the keys, seal them, the file keeps pointers. */
export function ImportSheet({ imp, keep, run }: { imp: ImportView; keep: Keep; run: Run }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), [imp.folder, imp.file]);
  const [chosen, setChosen] = useState(() => new Set(imp.vars.filter((v) => v.likely || v.kept_as).map((v) => v.var)));
  const [how, setHow] = useState<Keep>(keep === "once" ? "keep" : keep);
  const toggle = (name: string) =>
    setChosen((s) => {
      const next = new Set(s);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  const n = chosen.size;
  return (
    <div className="sheet" ref={sheet}>
      <Flow from={<FileCode2 size={30} strokeWidth={1.5} />} fromLabel={imp.file} to={<Vault size={30} strokeWidth={1.5} />} toLabel="Your keys" />
      <div className="center-text">
        <h2 className="title">Seal these keys</h2>
        <p className="help mono">{imp.folder}</p>
      </div>
      <div className="checklist" role="group" aria-label="Variables to import">
        {imp.vars.map((v) => (
          <label key={v.var} className={chosen.has(v.var) ? "on" : ""}>
            <input type="checkbox" checked={chosen.has(v.var)} onChange={() => toggle(v.var)} />
            <span className="box">{chosen.has(v.var) && <Check size={14} strokeWidth={2.5} />}</span>
            <span className="mono var">{v.var}</span>
            {v.kept_as && (
              <span className="tag" title={`You already keep this value as ${v.kept_as}`}>
                <Link2 size={13} strokeWidth={2} /> {v.kept_as}
              </span>
            )}
          </label>
        ))}
      </div>
      <div className="keep-buttons two" role="group" aria-label="How long to keep them">
        {KEEP_CHOICES.filter((c) => c.keep !== "once").map((c) => (
          <button key={c.keep} className={how === c.keep ? "pick" : ""} aria-pressed={how === c.keep} onClick={() => setHow(c.keep)}>
            <Icon name={c.icon} />
            {c.label}
          </button>
        ))}
      </div>
      <p className="help center">Unticked lines stay as they are. Ticked ones become <span className="mono">handoff:</span> pointers.</p>
      <span className="grow-space" />
      <div className="btn-row">
        <button className="btn secondary" onClick={() => void run(api.closeImport())}>Cancel</button>
        <button className="btn primary grow" disabled={n === 0} onClick={() => void run(api.finishImport([...chosen], how))}>
          <ShieldCheck size={18} strokeWidth={2} aria-hidden />
          {n === 0 ? "Pick a key" : `Seal ${n} ${n === 1 ? "key" : "keys"}`}
        </button>
      </div>
    </div>
  );
}

/** Using a key in code: the .env line, then the command. */
export function CodeSheet({ view, onClose }: { view: View; onClose: () => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => void enter(sheet.current, "rise", 360), []);
  const first = view.keys[0]?.label ?? "Dialpad";
  const pointer = /^[A-Za-z0-9_.:-]+$/.test(first) ? `handoff:${first}` : `"handoff:${first}"`;
  const command = "keyhandoff run -- python app.py";
  const [copied, setCopied] = useState(false);
  const copy = () => {
    // Only the command, never a key: it's safe on the clipboard.
    void navigator.clipboard?.writeText(command).then(() => setCopied(true));
  };
  return (
    <div className="sheet" ref={sheet}>
      <div className="center-text">
        <h2 className="title">Use a key in code</h2>
      </div>
      <ol className="pic-steps">
        <li>
          <span className="pic-n">1</span>
          <span className="pic-body">
            <span className="pic-head"><FileCode2 {...ICON} /> <span className="mono">.env</span></span>
            <span className="diff mono">
              <span className="del">API_KEY=sk_live_51Hq8v…</span>
              <span className="add">API_KEY={pointer}</span>
            </span>
          </span>
        </li>
        <li>
          <span className="pic-n">2</span>
          <span className="pic-body">
            <span className="pic-head"><SquareTerminal {...ICON} /> Any terminal</span>
            <button className="cmd mono" onClick={copy} title="Copy the command">
              <span>{command}</span>
              {copied ? <Check size={15} strokeWidth={2} /> : <Copy size={15} strokeWidth={1.75} />}
            </button>
          </span>
        </li>
        <li>
          <span className="pic-n">3</span>
          <span className="pic-body">
            <span className="pic-head"><KeyRound {...ICON} /> Allow once, here</span>
            <span className="help">Then it just runs. Printed keys show as hidden.</span>
          </span>
        </li>
      </ol>
      <span className="grow-space" />
      <button className="btn secondary" onClick={onClose}>Done</button>
    </div>
  );
}

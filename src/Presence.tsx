import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { FX, enter, type Fx } from "./motion";

const EASE_IN = "cubic-bezier(.4,0,.2,1)";
const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Show children with an eased entrance, and keep them on screen, inert, while
 * they ease back out after `show` turns false. Nothing appears or vanishes
 * in one frame. The last shown content is what leaves.
 */
export function Presence({ show, children, fx = "rise", inMs = 360, outMs = 240 }: { show: boolean; children: ReactNode; fx?: Fx; inMs?: number; outMs?: number }) {
  const last = useRef<ReactNode>(null);
  const box = useRef<HTMLSpanElement>(null);
  const [mounted, setMounted] = useState(show);
  if (show) last.current = children;

  useLayoutEffect(() => {
    const els = Array.from(box.current?.children ?? []) as HTMLElement[];
    if (show) {
      if (!mounted) { setMounted(true); return; }
      els.forEach((el) => { el.inert = false; el.getAnimations().forEach((a) => a.id === "leave" && a.cancel()); });
      els.forEach((el) => void enter(el, fx, inMs));
      return;
    }
    if (!mounted) return;
    if (reduced() || els.length === 0) { setMounted(false); return; }
    let live = true;
    const t = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--tempo")) || 1;
    const runs = els.flatMap((el) => {
      el.inert = true;
      const opts = { duration: outMs / t, easing: EASE_IN, fill: "forwards" as const };
      const a = el.animate([...FX[fx]].reverse(), opts);
      a.id = "leave";
      const all = [a];
      if (el instanceof HTMLDialogElement) {
        const b = el.animate([{ opacity: 1 }, { opacity: 0 }], { ...opts, pseudoElement: "::backdrop" });
        b.id = "leave";
        all.push(b);
      }
      return all;
    });
    void Promise.all(runs.map((a) => a.finished)).then(() => { if (live) setMounted(false); }, () => undefined);
    return () => { live = false; };
  }, [show, mounted]);

  if (!show && !mounted) return null;
  return <span ref={box} style={{ display: "contents" }}>{show ? children : last.current}</span>;
}

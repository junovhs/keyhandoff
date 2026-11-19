// Enter/leave motion: one vocabulary, every
// exit is its entrance played backwards, and durations divide by the CSS --tempo knob.

const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const EASE = "cubic-bezier(.22,1,.36,1)";
const EASE_IN = "cubic-bezier(.4,0,1,1)";

export const FX = {
  rise: [{ opacity: 0, transform: "translateY(12px)" }, { opacity: 1, transform: "none" }],
  pop: [{ opacity: 0, transform: "translateY(-8px) scale(.96)" }, { opacity: 1, transform: "none" }],
  fade: [{ opacity: 0 }, { opacity: 1 }],
  zoom: [{ opacity: 0, transform: "scale(.4)" }, { opacity: 1, transform: "none" }],
} satisfies Record<string, Keyframe[]>;

export type Fx = keyof typeof FX;

function tempo(): number {
  const t = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--tempo"));
  return t > 0 ? t : 1;
}

/** Play an element's entrance. Resolves when done; instant under reduced motion. */
export function enter(el: Element | null, fx: Fx = "rise", ms = 320, delay = 0): Promise<void> {
  if (!el || reduced()) return Promise.resolve();
  const t = tempo();
  const a = el.animate(FX[fx], { duration: ms / t, delay: delay / t, easing: EASE, fill: "backwards" });
  return a.finished.then(() => undefined, () => undefined);
}

/** Play an element's exit: its entrance backwards. */
export function leave(el: Element | null, fx: Fx = "rise", ms = 200): Promise<void> {
  if (!el || reduced()) return Promise.resolve();
  const a = el.animate([...FX[fx]].reverse(), { duration: ms / tempo(), easing: EASE_IN, fill: "forwards" });
  return a.finished.then(() => undefined, () => undefined);
}

/** Stagger the entrance of an element's children. */
export function stagger(parent: Element | null, fx: Fx = "rise", step = 45): void {
  if (!parent) return;
  // Wrappers that only group (display: contents) stagger their children instead.
  const kids = Array.from(parent.children).flatMap((c) => (getComputedStyle(c).display === "contents" ? Array.from(c.children) : [c]));
  kids.forEach((child, i) => void enter(child, fx, 320, i * step));
}

/** The pearl surfaces whose sheen follows the pointer. */
const SHEEN = ".btn.primary, .pocket-card, .card-file, .ledger-row, .segmented button.pick, .keep-buttons button.pick";

/**
 * Tell each pearl surface where the pointer is over it: --px/--py run 0..1
 * across its box. CSS turns that into the drifting colour, the glint and the
 * tilt. One listener for the whole window; writes at most once a frame.
 */
export function trackSheen(): () => void {
  let frame = 0;
  let last: PointerEvent | null = null;
  const paint = () => {
    frame = 0;
    const e = last;
    if (!e || !(e.target instanceof Element)) return;
    const el = e.target.closest<HTMLElement>(SHEEN);
    if (!el) return;
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;
    const x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
    el.style.setProperty("--px", x.toFixed(3));
    el.style.setProperty("--py", y.toFixed(3));
  };
  const onMove = (e: PointerEvent) => {
    if (e.pointerType === "touch") return;
    last = e;
    if (!frame) frame = requestAnimationFrame(paint);
  };
  window.addEventListener("pointermove", onMove, { passive: true });
  return () => { window.removeEventListener("pointermove", onMove); cancelAnimationFrame(frame); };
}

/**
 * Move elements to a new layout smoothly (FLIP): note where they are, make
 * the change, then glide each from its old box to its new one. Anything
 * already gliding starts from wherever it is on screen now.
 */
export function flip(els: HTMLElement[], change: () => void, ms = 420): void {
  const before = els.map((el) => [el, el.getBoundingClientRect()] as const);
  els.forEach((el) => el.getAnimations().forEach((a) => a.id === "flip" && a.cancel()));
  change();
  if (reduced()) return;
  const t = tempo();
  for (const [el, a] of before) {
    if (!el.isConnected) continue;
    const b = el.getBoundingClientRect();
    if (!a.width || !b.width) continue;
    const dx = a.left - b.left, dy = a.top - b.top;
    if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && Math.abs(a.width - b.width) < 1 && Math.abs(a.height - b.height) < 1) continue;
    const anim = el.animate(
      [
        // No width or height at the end: the glide heads for the live layout,
        // so a window still being resized is followed instead of snapped to.
        { transform: `translate(${dx}px, ${dy}px)`, width: `${a.width}px`, height: `${a.height}px` },
        { transform: "translate(0, 0)" },
      ],
      { duration: ms / t, easing: EASE },
    );
    anim.id = "flip";
  }
}

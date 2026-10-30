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
  Array.from(parent.children).forEach((child, i) => void enter(child, fx, 320, i * step));
}

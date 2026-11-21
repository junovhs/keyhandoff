// Scroll reveals, the blind-spot diagram, Just you, the team handoff and how teams work. Everything here
// is decoration: without it the page shows its full content.
const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const reveal = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      e.target.classList.add("in");
      reveal.unobserve(e.target);
    }
  },
  { rootMargin: "0px 0px -8% 0px" },
);
document.querySelectorAll(".rv").forEach((el) => (reduced ? el.classList.add("in") : reveal.observe(el)));

const solo = document.getElementById("solo-stage");
// Safe Copy listening, caught, the vault, then KeyHandoff typing the key in.
if (solo) runFrames(solo, [["0", 2200], ["1", 3000], ["2", 2400], ["3", 6400]], "3", soloTyping(solo));
const circle = document.getElementById("circle-stage");
// Ben has the keys, invites Ann and Priya, they join and get them, he changes one, then Priya leaves.
if (circle) runFrames(circle, [["0", 2600], ["1", 3000], ["2", 3600], ["3", 3600], ["4", 4800]], "4");
const standin = document.getElementById("sw-stage");
// The stand-in, the real key going in on the way to Stripe, a package sending the stand-in out, then the tripwire.
if (standin) runFrames(standin, [["0", 2800], ["1", 3600], ["2", 2600], ["3", 4400]], "3");
const duo = document.getElementById("duo");
// An opening line, three steps in Ben's window, two in Ann's, then a closing line.
if (duo) runFrames(duo, [["intro", 3600], ["0", 2600], ["1", 2400], ["2", 3400], ["3", 2800], ["4", 3000], ["outro", 4000]], "4");

/** Just you's last beat: the cursor clicks the field, then the key's Type button, then KeyHandoff types. */
function soloTyping(stage: HTMLElement) {
  const cursor = document.getElementById("solo-cursor")!;
  const field = document.getElementById("solo-field")!;
  const button = document.getElementById("solo-type")!;
  let timers: number[] = [];
  const later = (ms: number, fn: () => void) => timers.push(window.setTimeout(fn, ms));
  // The cursor's tip is its top-left corner, so this puts the tip on the element's centre.
  const moveTo = (el: HTMLElement, dx = 0, dy = 0) => {
    const r = el.getBoundingClientRect();
    const s = stage.getBoundingClientRect();
    cursor.style.transform = `translate(${r.left - s.left + r.width / 2 + dx}px, ${r.top - s.top + r.height / 2 + dy}px)`;
  };
  return (step: string) => {
    timers.forEach(clearTimeout);
    timers = [];
    stage.classList.remove("focused", "typing");
    button.classList.remove("clicked");
    cursor.classList.remove("show");
    if (step !== "3") return;
    cursor.style.transition = "none";
    moveTo(field, 70, 90);
    void cursor.getBoundingClientRect(); // apply the jump before the transition comes back
    cursor.style.transition = "";
    later(600, () => {
      cursor.classList.add("show");
      moveTo(field);
    });
    later(1600, () => stage.classList.add("focused"));
    later(2000, () => moveTo(button));
    later(3000, () => button.classList.add("clicked"));
    later(3200, () => stage.classList.add("typing"));
    later(5200, () => cursor.classList.remove("show"));
  };
}

/** Plays a section as timed frames by setting data-step, looping while it's seen and starting
 *  over each time it comes into view. With reduced motion it holds on one frame. */
function runFrames(root: HTMLElement, steps: [string, number][], still: string, onStep?: (step: string) => void) {
  const show = (i: number) => {
    root.dataset.step = steps[i][0];
    onStep?.(steps[i][0]);
  };
  if (reduced) return void (root.dataset.step = still);
  let step = 0;
  let timer = 0;
  const next = () => {
    step = (step + 1) % steps.length;
    show(step);
    timer = window.setTimeout(next, steps[step][1]);
  };
  new IntersectionObserver(([e]) => {
    clearTimeout(timer);
    if (!e.isIntersecting) return;
    step = 0;
    show(0);
    timer = window.setTimeout(next, steps[0][1]);
  }, { threshold: 0.4 }).observe(root);
}

type State = "exposed" | "safe";

/** The blind-spot diagram and the line under it: everyone gets the key, then KeyHandoff turns it into a pointer.
 *  It flips once on its own after it's been seen; the switch below it flips it either way. */
function runReaders(root: HTMLElement) {
  const value = document.getElementById("env-val")!;
  const punch = document.getElementById("punch")!;
  const lines = Array.from(punch.querySelectorAll<HTMLElement>("[data-ok]"));
  const buttons = Array.from(document.querySelectorAll<HTMLButtonElement>(".readers-switch [data-to]"));
  let timer = 0;
  const set = (state: State) => {
    root.dataset.state = state;
    for (const b of buttons) b.setAttribute("aria-pressed", String(b.dataset.to === state));
    punch.dataset.state = state;
    const pairs: [HTMLElement, string][] = [
      [value, state === "safe" ? value.dataset.pointer! : value.dataset.key!],
      ...lines.map((l): [HTMLElement, string] => [l, state === "safe" ? l.dataset.ok! : l.dataset.bad!]),
    ];
    for (const [el, text] of pairs) {
      if (reduced) el.textContent = text;
      else scramble(el, text);
    }
  };
  for (const b of buttons) b.addEventListener("click", () => {
    clearTimeout(timer);
    set(b.dataset.to as State);
  });
  const seen = new IntersectionObserver(([e]) => {
    if (!e.isIntersecting) return;
    seen.disconnect();
    timer = window.setTimeout(() => set("safe"), 4000);
  }, { threshold: 0.6 });
  seen.observe(root);
}


const GLYPHS = "abcdefghijkmnpqrstuvwxyz0123456789";
const rolling = new WeakMap<HTMLElement, string>();

/** Rolls an element's text over to a new value, left to right. A newer roll replaces it. */
function scramble(el: HTMLElement, to: string) {
  rolling.set(el, to);
  const start = performance.now();
  const length = Math.max(el.textContent?.length ?? 0, to.length);
  const step = (now: number) => {
    if (rolling.get(el) !== to) return;
    const done = Math.floor(((now - start) / 700) * length);
    let out = to.slice(0, done);
    for (let i = done; i < Math.min(length, to.length + 2); i++) out += GLYPHS[Math.floor(Math.random() * GLYPHS.length)];
    el.textContent = done >= length ? to : out;
    if (done < length) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

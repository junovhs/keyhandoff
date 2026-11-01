// Scroll reveals, the hero's .env search, the blind-spot diagram, Just you and the team handoff. Everything here
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

const scan = document.getElementById("scan");
if (scan) runScan(scan);
const readers = document.getElementById("readers");
if (readers) runReaders(readers);
const solo = document.getElementById("solo-stage");
// Safe Copy listening, caught, the vault, then KeyHandoff typing the key in.
if (solo) runFrames(solo, [["0", 2200], ["1", 3000], ["2", 2400], ["3", 6400]], "3", soloTyping(solo));
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

/** Plays the hero in four steps: close up on typing .env, pull back to the keys it finds, a cursor
 *  turns KeyHandoff on, and the keys become pointers. It plays once when seen, then on Replay. */
function runScan(root: HTMLElement) {
  const q = document.getElementById("scan-q")!;
  const rows = Array.from(root.querySelectorAll<HTMLElement>(".row"));
  const values = Array.from(root.querySelectorAll<HTMLElement>(".v"));
  const toggle = document.getElementById("kh-switch")!;
  const cursor = document.getElementById("scan-cursor")!;
  let run = 0;

  // Positions are inside the window, so the hero's tilt carries the cursor with it.
  const moveCursor = (x: number, y: number) => (cursor.style.transform = `translate(${x}px, ${y}px)`);
  const parkCursor = () => {
    cursor.style.transition = "none";
    moveCursor(toggle.offsetLeft + toggle.offsetWidth + 40, toggle.offsetTop + 120);
    void cursor.offsetWidth; // apply the jump before the transition comes back
    cursor.style.transition = "";
  };

  const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

  function setState(state: State, animate: boolean) {
    root.dataset.state = state;
    for (const v of values) {
      const text = state === "safe" ? v.dataset.pointer! : v.dataset.key!;
      if (animate) scramble(v, text);
      else {
        rolling.set(v, text);
        v.textContent = text;
      }
    }
  }

  async function play(id: number) {
    setState("exposed", false);
    toggle.classList.remove("on");
    cursor.classList.remove("show");
    parkCursor();
    root.classList.remove("found");
    rows.forEach((r) => r.classList.remove("on"));
    q.textContent = "";
    root.classList.add("zoom", "pressing");
    await wait(800);
    if (id !== run) return;
    root.classList.remove("pressing");
    for (const ch of ".env") {
      q.textContent += ch;
      await wait(220);
      if (id !== run) return;
    }
    await wait(600);
    if (id !== run) return;
    root.classList.remove("zoom");
    await wait(700);
    for (const r of rows) {
      if (id !== run) return;
      r.classList.add("on");
      await wait(90);
    }
    root.classList.add("found");
    await wait(2200);
    if (id !== run) return;
    cursor.classList.add("show");
    moveCursor(toggle.offsetLeft + toggle.offsetWidth - 22, toggle.offsetTop + toggle.offsetHeight / 2);
    await wait(1000);
    if (id !== run) return;
    toggle.classList.add("press");
    await wait(140);
    toggle.classList.remove("press");
    toggle.classList.add("on");
    setState("safe", true);
    await wait(900);
    cursor.classList.remove("show");
  }

  rows.forEach((r) => r.classList.add("on"));
  root.classList.add("found");
  if (reduced) {
    // No zoom or typing: the keys, then the pointers.
    setTimeout(() => {
      toggle.classList.add("on");
      setState("safe", false);
    }, 3000);
    return;
  }
  document.getElementById("scan-again")!.addEventListener("click", () => void play(++run));
  const start = new IntersectionObserver(([e]) => {
    if (!e.isIntersecting) return;
    start.disconnect();
    void play(++run);
  }, { threshold: 0.4 });
  start.observe(root);
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

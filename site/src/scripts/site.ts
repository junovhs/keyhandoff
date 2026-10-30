// Scroll reveals, the hero's .env search and the attack replay. Everything here
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
const replay = document.getElementById("replay-stage");
if (replay) runReplay(replay);

document.querySelectorAll<HTMLButtonElement>(".copy").forEach((b) => {
  const label = b.querySelector("span")!;
  b.addEventListener("click", async () => {
    await navigator.clipboard.writeText(b.dataset.copy!);
    b.classList.add("done");
    label.textContent = "Copied";
    setTimeout(() => {
      b.classList.remove("done");
      label.textContent = "Copy";
    }, 1600);
  });
});

/** Plays the attack line by line the first time it scrolls into view. */
function runReplay(root: HTMLElement) {
  const lines = Array.from(root.querySelectorAll<HTMLElement>(".line"));
  const clock = document.getElementById("replay-clock")!;
  const END_MS = 8400;
  // The clock runs on the install's 9 seconds, not the replay's own timing.
  const CLOCK_RATE = 9 / (END_MS / 1000);
  let frame = 0;

  const show = (s: number) => {
    clock.textContent = `00:${String(Math.floor(s)).padStart(2, "0")}`;
  };
  function play() {
    cancelAnimationFrame(frame);
    root.classList.add("ready", "playing");
    lines.forEach((l) => l.classList.remove("on"));
    const start = performance.now();
    const tick = (now: number) => {
      const t = now - start;
      show(Math.min(9, (t / 1000) * CLOCK_RATE));
      for (const l of lines) {
        if (t >= Number(l.dataset.at) && !l.classList.contains("on")) l.classList.add("on");
      }
      if (t < END_MS) frame = requestAnimationFrame(tick);
      else root.classList.remove("playing");
    };
    frame = requestAnimationFrame(tick);
  }

  if (reduced) return; // every line stays visible, with the clock at 00:09
  document.getElementById("replay-again")!.addEventListener("click", play);
  root.classList.add("ready");
  show(0);
  const start = new IntersectionObserver(([e]) => {
    if (!e.isIntersecting) return;
    start.disconnect();
    play();
  }, { threshold: 0.45 });
  start.observe(root);
}

type State = "exposed" | "safe";

/** Searches for .env and finds the keys, then shows KeyHandoff turning them into pointers. */
function runScan(root: HTMLElement) {
  const q = document.getElementById("scan-q")!;
  const rows = Array.from(root.querySelectorAll<HTMLElement>(".row"));
  const values = Array.from(root.querySelectorAll<HTMLElement>(".v"));
  const buttons = Array.from(root.querySelectorAll<HTMLButtonElement>("[data-to]"));
  let run = 0;
  let visible = true;

  const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));
  const whileVisible = async () => {
    while (!visible) await wait(250);
  };

  function setState(state: State, animate: boolean) {
    root.dataset.state = state;
    for (const b of buttons) b.setAttribute("aria-pressed", String(b.dataset.to === state));
    for (const v of values) {
      const text = state === "safe" ? v.dataset.pointer! : v.dataset.key!;
      if (animate && !reduced) scramble(v, text);
      else {
        rolling.set(v, text);
        v.textContent = text;
      }
    }
  }

  async function loop(id: number) {
    while (id === run) {
      await whileVisible();
      setState("exposed", false);
      root.classList.remove("found");
      rows.forEach((r) => r.classList.remove("on"));
      q.textContent = "";
      root.classList.add("pressing");
      await wait(700);
      if (id !== run) return;
      root.classList.remove("pressing");
      for (const ch of ".env") {
        q.textContent += ch;
        await wait(130);
        if (id !== run) return;
      }
      await wait(250);
      for (const r of rows) {
        if (id !== run) return;
        r.classList.add("on");
        await wait(70);
      }
      if (id !== run) return;
      root.classList.add("found");
      await wait(3600);
      if (id !== run) return;
      await whileVisible();
      setState("safe", true);
      await wait(5200);
    }
  }

  for (const b of buttons) {
    b.addEventListener("click", () => {
      run++; // the viewer took over; the loop stops
      q.textContent = ".env";
      rows.forEach((r) => r.classList.add("on"));
      root.classList.add("found");
      root.classList.remove("pressing");
      setState(b.dataset.to as State, true);
    });
  }

  new IntersectionObserver(([e]) => (visible = e.isIntersecting)).observe(root);
  rows.forEach((r) => r.classList.add("on"));
  root.classList.add("found");
  if (!reduced) void loop(++run);
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

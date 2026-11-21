// The hero's vault window reads as it stands. Then a cursor works it through each
// bullet beside it, on a loop: save a key with Safe Copy, send one sealed for Ann
// (dragged into a chat, an email or a folder, or copied as text),
// a program asks for a key, an AI agent asks for one. The bullet being shown lights up.

type Step = { view?: string; point?: 0 | 1 | 2; click?: string; drop?: true; wait: number; fresh?: boolean; shield?: boolean };

const STEPS: Step[] = [
  // Save, use and share keys safely.
  { view: "", point: 0, fresh: false, wait: 500 },
  { click: "[data-hv-add]", wait: 900 },
  { view: "copy", wait: 1300 },
  { view: "caught", wait: 900 },
  { click: "[data-hv-keep]", wait: 900 },
  { view: "", fresh: true, shield: true, wait: 2200 },
  { click: "[data-hv-send]", wait: 900 },
  { view: "sealed", wait: 900 },
  { drop: true, wait: 2600 },
  { view: "", wait: 900 },
  // Your programs never hold the real key.
  { view: "program", point: 1, wait: 1500 },
  { click: "[data-hv-always]", wait: 1100 },
  { view: "", wait: 900 },
  // AI agents have to ask you first.
  { view: "agent", point: 2, wait: 1700 },
  { click: "[data-hv-session]", wait: 1100 },
  { view: "", wait: 1600 },
];

const app = document.querySelector<HTMLElement>("[data-hero-app]");
const points = [...document.querySelectorAll<HTMLElement>("[data-points] li")];
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;

if (app && !reduced) {
  const cursor = app.querySelector<SVGElement>(".hv-cursor")!;
  const fresh = app.querySelector<HTMLElement>(".fresh");
  const shield = app.querySelector<SVGElement>("[data-hv-shield]");
  let i = 0, timer = 0, visible = false, started = false;

  const ghost = app.querySelector<HTMLElement>(".hv-ghost")!;
  const targets = [...app.querySelectorAll<HTMLElement>("[data-hv-to]")];
  let loop = 0;
  const at = (el: Element, fx = 0.6, fy = 0.55) => {
    const a = app.getBoundingClientRect(), r = el.getBoundingClientRect();
    return [r.left - a.left + r.width * fx, r.top - a.top + r.height * fy];
  };
  const put = (el: Element, [x, y]: number[]) => ((el as HTMLElement).style.transform = `translate(${x}px, ${y}px)`);
  /** The sealed file goes somewhere: dragged to a chat, an email or a folder, or copied as text. */
  const drop = () => {
    const to = targets[loop++ % targets.length];
    if (to.dataset.hvTo === "clip") {
      moveTo("[data-hv-copyfile]");
      setTimeout(() => to.classList.add("hit"), 700);
    } else {
      const file = app.querySelector("[data-hv-file]")!;
      put(cursor, at(file, 0.3, 0.5));
      setTimeout(() => {
        cursor.classList.add("press");
        ghost.style.transition = "none";
        put(ghost, at(file, 0.08, 0.2));
        void ghost.offsetWidth;
        ghost.style.transition = "";
        ghost.classList.add("show");
        setTimeout(() => { put(cursor, at(to, 0.5, 0.5)); put(ghost, at(to, 0.22, 0.25)); }, 120);
        setTimeout(() => { cursor.classList.remove("press"); ghost.classList.remove("show"); to.classList.add("hit"); }, 1000);
      }, 650);
    }
    setTimeout(() => to.classList.remove("hit"), 2500);
  };

  const point = (n: number) => points.forEach((p, k) => p.classList.toggle("on", k === n));
  const moveTo = (sel: string) => {
    const t = app.querySelector<HTMLElement>(sel);
    if (!t) return;
    const a = app.getBoundingClientRect(), r = t.getBoundingClientRect();
    cursor.style.transform = `translate(${r.left - a.left + r.width * 0.6}px, ${r.top - a.top + r.height * 0.55}px)`;
    cursor.classList.add("show");
    setTimeout(() => {
      cursor.classList.add("press");
      t.classList.add("hv-press");
      setTimeout(() => { cursor.classList.remove("press"); t.classList.remove("hv-press"); }, 260);
    }, 620);
  };

  const run = () => {
    if (!visible) return;
    const s = STEPS[i];
    if (s.view !== undefined) {
      app.dataset.hv = s.view;
      if (s.view) cursor.classList.add("show");
    }
    if (s.point !== undefined) point(s.point);
    if (s.fresh !== undefined) fresh?.classList.toggle("hv-hide", !s.fresh);
    if (s.shield) { shield?.classList.remove("hv-glow"); void shield?.getBoundingClientRect(); shield?.classList.add("hv-glow"); }
    if (s.click) moveTo(s.click);
    if (s.drop) drop();
    i = (i + 1) % STEPS.length;
    timer = window.setTimeout(run, s.wait);
  };

  // Park the cursor below the window; it plays only while the hero is on screen.
  cursor.style.transform = `translate(${app.offsetWidth * 0.75}px, ${app.offsetHeight + 40}px)`;
  new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    clearTimeout(timer);
    if (visible) timer = window.setTimeout(run, started ? 300 : 1400);
    started = true;
  }, { threshold: 0.4 }).observe(app);
}

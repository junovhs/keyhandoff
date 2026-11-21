// The key is the homepage's character. A solid holographic KeyHandoff logo
// key is born large in the hero's halo ([data-born]) and follows the reader
// down on long swooping curves. When the reader pauses at a section, the key
// docks into that section's scene ([data-scene]) and acts it out: each step
// sets data-step and cumulative classes (s1, s2...) on the scene for its
// props, and moves the key to a pose given in % of the scene. Scrolling again
// stops the scene and the key comes along. It ends in the download logo
// ([data-home]).
const SVG = "http://www.w3.org/2000/svg";
// The key from the KeyHandoff logo (Monolith, as Logo.astro draws it).
const KEY = `url("data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="28 8 64 104"><path fill-rule="evenodd" d="M28 8H92V60H72V78H90V90H72V98H90V112H48V60H28ZM50 24V44H70V24Z"/></svg>',
)}")`;
// Tail layers [class, length, gap behind the key]: brighter further back, so
// the line fades out before the key. It only shows while the key moves.
const TAILS = [["t1", 560, 120], ["t2", 440, 200], ["t3", 320, 290]] as const;

/** A pose in a scene: x and y in % of the scene, rotation (0 points the key
 *  down, -90 lays it pointing right), scale, and looks (plain, standin, sealed, shiver). */
type Pose = [x: number, y: number, r: number, s: number, look?: string];
type Step = { d: number; k: Pose };
const SCENES: Record<string, Step[]> = {
  // Threats close in on a bare key in a .env; a shield turns them away.
  threats: [
    { d: 900, k: [50, 53, 0, 0.7, "plain"] },
    { d: 1900, k: [50, 53, 0, 0.7, "plain shiver"] },
    { d: 400, k: [50, 53, 0, 0.75, "plain"] },
    { d: 0, k: [50, 53, 0, 0.8] },
  ],
  // Copied on the provider's page, caught off the clipboard, sealed in the vault.
  save: [
    { d: 1000, k: [20, 53, -90, 0.42, "plain"] },
    { d: 1300, k: [50, 24, -90, 0.42, "plain"] },
    { d: 1000, k: [80, 40, 0, 0.6] },
    { d: 0, k: [80, 64, 0, 0.5] },
  ],
  // A program asks, you allow it, the key goes into the program.
  use: [
    { d: 900, k: [88, 14, 0, 0.4] },
    { d: 900, k: [84, 26, 0, 0.5] },
    { d: 2100, k: [84, 26, 0, 0.5] },
    { d: 0, k: [22, 63, -90, 0.42] },
  ],
  // A stand-in stays in the program; the real key rides the encrypted tunnel.
  standin: [
    { d: 900, k: [20, 61, -90, 0.42] },
    { d: 1100, k: [32, 42, -78, 0.55] },
    { d: 1500, k: [78, 27, -102, 0.5] },
    { d: 0, k: [84, 44, 0, 0.4] },
  ],
  // An agent reaches; a gate holds; you allow one session; then revoke it.
  agent: [
    { d: 800, k: [62, 50, 0, 0.7] },
    { d: 1400, k: [68, 52, 14, 0.7, "shiver"] },
    { d: 2100, k: [68, 52, 0, 0.7] },
    { d: 2000, k: [24, 36, -30, 0.42] },
    { d: 0, k: [62, 50, 0, 0.7] },
  ],
  // Sealed for Ann, across the chat unreadable, opened only by her.
  share: [
    { d: 900, k: [21, 50, 0, 0.6] },
    { d: 900, k: [27, 50, 0, 0.6, "sealed"] },
    { d: 2100, k: [73, 50, 0, 0.6, "sealed"] },
    { d: 0, k: [79, 50, 0, 0.6] },
  ],
  // The stand-in is yanked by a package, stopped at the tripwire, team covered.
  leak: [
    { d: 900, k: [16, 63, -90, 0.4, "standin"] },
    { d: 900, k: [34, 30, -60, 0.48, "standin"] },
    { d: 650, k: [61, 46, -90, 0.48, "standin"] },
    { d: 1500, k: [52, 54, 35, 0.48, "standin shiver"] },
    { d: 0, k: [16, 63, -90, 0.4] },
  ],
  // Down through what you see, the Rust core, and the lock.
  locked: [
    { d: 900, k: [22, 6, 0, 0.55] },
    { d: 1400, k: [22, 26, 0, 0.55] },
    { d: 1400, k: [22, 52, 0, 0.5] },
    { d: 0, k: [22, 78, 0, 0.45] },
  ],
};
const LOOKS = ["plain", "standin", "sealed", "shiver"];

const bornAt = document.querySelector<HTMLElement>("[data-born]");
const home = document.querySelector<HTMLElement>("[data-home]");
const scenes = [...document.querySelectorAll<HTMLElement>("[data-scene]")].filter((s) => SCENES[s.dataset.scene!]);
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
if (bornAt && home && scenes.length) init(bornAt, home);

function init(bornEl: HTMLElement, homeEl: HTMLElement) {
  const root = document.createElement("div");
  root.className = "keypath";
  root.setAttribute("aria-hidden", "true");
  root.style.setProperty("--key-mask", KEY);
  document.documentElement.style.setProperty("--key-mask", KEY);
  const sparks = Array.from({ length: 14 }, (_, i) => {
    const a = (i / 14) * Math.PI * 2, r = 110 + (i % 3) * 40;
    return `<i class="kp-spark" style="--sx:${Math.cos(a) * r}px;--sy:${Math.sin(a) * r}px"></i>`;
  }).join("");
  root.innerHTML =
    `<svg class="kp-svg"><defs><linearGradient id="kp-pearl" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="600" y2="600" spreadMethod="reflect"><stop offset="0" stop-color="#ddd5ff"/><stop offset=".33" stop-color="#c4ece6"/><stop offset=".66" stop-color="#f4d4e8"/><stop offset="1" stop-color="#d3cdff"/></linearGradient></defs>` +
    `<path class="kp-route" fill="none" stroke="none"/>${TAILS.map(([c]) => `<path class="kp-tail ${c}"/>`).join("")}</svg>` +
    `<div class="kp-burst"><i class="kp-flash"></i><i class="kp-ring"></i><i class="kp-ring r2"></i>${sparks}</div>` +
    `<div class="kp-key"><div class="kp-spin"><i class="kp-aura"></i><span class="kp-capsule"></span><div class="kp-body"><i class="kp-shape"></i><i class="kp-plain"></i><i class="kp-glint"></i></div></div></div>`;
  document.body.prepend(root);
  const svg = root.querySelector("svg")!;
  const route = root.querySelector<SVGPathElement>(".kp-route")!;
  const tails = TAILS.map(([c, len, gap]) => ({ el: root.querySelector<SVGPathElement>(`.kp-tail.${c}`)!, len, gap }));
  const burst = root.querySelector<HTMLElement>(".kp-burst")!;
  const key = root.querySelector<HTMLElement>(".kp-key")!;
  const spin = root.querySelector<HTMLElement>(".kp-spin")!;

  type Pt = { x: number; y: number };
  let pts: Pt[] = []; // birth, each scene's dock, home
  let total = 0;
  let table: { len: number; y: number }[] = [];
  let lens: number[] = [];
  let along = 0; // smoothed length along the path
  // The key's drawn state, eased towards its goal every frame.
  const cur = { x: 0, y: 0, r: 0, s: 1 };
  let trail = 0, idle = 0;
  let playing: { el: HTMLElement; steps: Step[]; i: number; at: number; scroll: number } | null = null;
  let lastScroll = 0;
  let look = "";

  const sceneBox = (el: HTMLElement) => {
    const r = el.getBoundingClientRect();
    return { x: r.left + scrollX, y: r.top + scrollY, w: r.width, h: r.height };
  };
  const posePt = (el: HTMLElement, k: Pose): Pt => {
    const b = sceneBox(el);
    return { x: b.x + (b.w * k[0]) / 100, y: b.y + (b.h * k[1]) / 100 };
  };
  const scaleFor = () => (innerWidth < 760 ? 0.7 : 1);
  // In the hero the key is the one thing to look at, sized to its halo.
  const heroScale = () => (bornEl.offsetWidth * 0.44) / 84;
  /** Travelling size: hero size at birth, shrinking to normal over the first half of the first leg. */
  const travelScale = () => {
    const t = Math.min(1, along / Math.max(1, (lens[1] ?? 1) * 0.5));
    const e = 1 - (1 - t) ** 3;
    return heroScale() + (scaleFor() - heroScale()) * e;
  };
  // On narrow screens each scene keeps its 1040px layout and shrinks to fit.
  const DESIGN_W = 1040;
  const shrink = new Map<HTMLElement, number>();
  function fitScenes() {
    for (const s of scenes) {
      if (s.dataset.scene === "locked") continue;
      const avail = s.parentElement!.clientWidth;
      const k = Math.min(1, avail / DESIGN_W);
      if (shrink.get(s) === k) continue;
      shrink.set(s, k);
      s.style.width = k < 1 ? `${DESIGN_W}px` : "";
      s.style.transformOrigin = "0 0";
      s.style.transform = k < 1 ? `scale(${k})` : "";
      s.style.marginBottom = k < 1 ? `${-s.offsetHeight * (1 - k)}px` : "";
    }
  }

  function layout() {
    fitScenes();
    const docH = document.documentElement.scrollHeight;
    root.style.height = `${docH}px`;
    const b = bornEl.getBoundingClientRect();
    const h = homeEl.getBoundingClientRect();
    pts = [
      { x: b.left + b.width / 2, y: b.top + scrollY + b.height / 2 },
      ...scenes.map((s) => posePt(s, SCENES[s.dataset.scene!][0].k)),
      { x: h.left + h.width / 2, y: h.top + scrollY + h.height / 2 },
    ];
    // One smooth spline through every point, with stretched tangents for long swoops.
    const P = [{ x: pts[0].x, y: pts[0].y - 1 }, ...pts, { x: pts[pts.length - 1].x, y: pts[pts.length - 1].y + 1 }];
    const k = 1.7 / 6;
    const legs: string[] = [];
    for (let i = 1; i < P.length - 2; i++) {
      const p0 = P[i - 1], p1 = P[i], p2 = P[i + 1], p3 = P[i + 2];
      legs.push(`C ${p1.x + (p2.x - p0.x) * k} ${p1.y + (p2.y - p0.y) * k}, ${p2.x - (p3.x - p1.x) * k} ${p2.y - (p3.y - p1.y) * k}, ${p2.x} ${p2.y}`);
    }
    const d = `M ${pts[0].x} ${pts[0].y} ${legs.join(" ")}`;
    route.setAttribute("d", d);
    for (const t of tails) t.el.setAttribute("d", d);
    total = route.getTotalLength();
    const probe = document.createElementNS(SVG, "path");
    svg.append(probe);
    lens = [0];
    for (let i = 1; i <= legs.length; i++) {
      probe.setAttribute("d", `M ${pts[0].x} ${pts[0].y} ${legs.slice(0, i).join(" ")}`);
      lens.push(probe.getTotalLength());
    }
    probe.remove();
    table = [];
    for (let len = 0; len <= total; len += 8) table.push({ len, y: route.getPointAtLength(len).y });
    svg.setAttribute("viewBox", `0 0 ${innerWidth} ${docH}`);
    burst.style.transform = `translate(${pts[0].x}px, ${pts[0].y}px)`;
    if (!cur.x) {
      along = target();
      const p = route.getPointAtLength(along);
      Object.assign(cur, { x: p.x, y: p.y, r: 0, s: travelScale() });
    }
    kick();
  }

  /** Where along the path the key belongs for this scroll position: resting
   *  at each point for the first third of the way to the next, then moving,
   *  and never off screen. */
  function target() {
    const anchor = scrollY + innerHeight * 0.5;
    let len = total;
    if (anchor <= pts[0].y) len = 0;
    else
      for (let i = 0; i < pts.length - 1; i++) {
        const a = pts[i].y, b = pts[i + 1].y;
        if (anchor > b) continue;
        const t = (anchor - a) / (b - a), hold = 0.3;
        const m = t < hold ? 0 : (t - hold) / (1 - hold);
        const e = m < 0.5 ? 4 * m * m * m : 1 - (-2 * m + 2) ** 3 / 2;
        len = lens[i] + (lens[i + 1] - lens[i]) * e;
        break;
      }
    const top = scrollY + 200, bottom = scrollY + innerHeight - 120;
    let i = Math.min(table.length - 1, Math.round(len / 8));
    if (table[i].y < top) {
      while (i < table.length - 1 && table[i].y < top) i++;
      return Math.max(len, table[i].len);
    }
    if (table[i].y > bottom) {
      while (i > 0 && table[i].y > bottom) i--;
      return Math.min(len, table[i].len);
    }
    return len;
  }

  function setLook(next: string) {
    if (next === look) return;
    look = next;
    const on = next.split(" ").filter(Boolean);
    for (const l of LOOKS) key.classList.toggle(l, on.includes(l));
  }

  function setStep(el: HTMLElement, i: number) {
    el.dataset.step = `${i}`;
    for (let n = 1; n <= 6; n++) el.classList.toggle(`s${n}`, n <= i);
  }

  /** The scene in the middle of the screen, if the reader has paused on one. */
  function sceneHere() {
    let best: HTMLElement | null = null, dist = Infinity;
    for (const s of scenes) {
      const r = s.getBoundingClientRect();
      const mid = r.top + r.height / 2;
      if (r.bottom < innerHeight * 0.2 || r.top > innerHeight * 0.8) continue;
      const d = Math.abs(mid - innerHeight / 2);
      if (d < dist) (best = s), (dist = d);
    }
    return best;
  }

  function play(el: HTMLElement) {
    const steps = SCENES[el.dataset.scene!];
    playing = { el, steps, i: 0, at: performance.now(), scroll: scrollY };
    setStep(el, 0);
    if (reduced) {
      playing.i = steps.length - 1;
      setStep(el, playing.i);
    }
  }

  function stop() {
    if (!playing) return;
    setStep(playing.el, 0);
    playing = null;
  }

  let running = false;
  function frame() {
    const now = performance.now();
    let goal: { x: number; y: number; r: number; s: number };
    let wantLook = "";
    // Paused on a scene: dock and act it out. Scrolling again ends it.
    if (!playing && now - lastScroll > 450) {
      const s = sceneHere();
      if (s) play(s);
    }
    if (playing) {
      const p = playing;
      while (p.steps[p.i].d && now - p.at > p.steps[p.i].d && p.i < p.steps.length - 1) {
        p.at += p.steps[p.i].d;
        p.i++;
        setStep(p.el, p.i);
      }
      const k = p.steps[p.i].k;
      const pt = posePt(p.el, k);
      goal = { x: pt.x, y: pt.y, r: k[2], s: k[3] * 1.4 * (shrink.get(p.el) ?? 1) }; // the star of its scene
      wantLook = k[4] ?? "";
    } else {
      along += reduced ? target() - along : (target() - along) * 0.09;
      const p = route.getPointAtLength(along);
      const q = route.getPointAtLength(Math.min(total, along + 2));
      const r = along >= total - 1 ? 0 : (Math.atan2(q.y - p.y, q.x - p.x) * 180) / Math.PI - 90;
      goal = { x: p.x, y: p.y, r, s: travelScale() };
    }
    setLook(wantLook);
    const ease = reduced ? 1 : playing ? 0.11 : 0.2;
    const dx = goal.x - cur.x, dy = goal.y - cur.y;
    cur.x += dx * ease;
    cur.y += dy * ease;
    let dr = goal.r - cur.r;
    dr = ((dr + 540) % 360) - 180;
    cur.r += dr * (reduced ? 1 : 0.14);
    cur.s += (goal.s - cur.s) * (reduced ? 1 : 0.14);
    const speed = Math.hypot(dx, dy) * ease;
    // Fairy float while nothing moves: a slow bob, a little sway, a slight tilt.
    idle = reduced ? 0 : speed > 0.6 ? Math.max(0, idle - 0.08) : Math.min(1, idle + 0.012);
    const t = now / 1000;
    const amp = playing ? 0.4 : 1;
    const bob = Math.sin(t * 1.7) * 9 * idle * amp, sway = Math.sin(t * 0.83 + 1) * 7 * idle * amp, tilt = Math.sin(t * 1.1) * 7 * idle * amp;
    const landed = !playing && along >= total - 2;
    key.classList.toggle("landed", landed);
    homeEl.classList.toggle("kp-home", landed);
    key.style.transform = `translate(${cur.x + sway}px, ${cur.y + bob}px) scale(${cur.s})`;
    spin.style.transform = `rotate(${cur.r + tilt}deg)`;
    // The comet tail only follows the key along the path, while it travels.
    trail = reduced || playing ? Math.max(0, trail - 0.08) : Math.max(Math.min(1, speed / 6), trail - 0.025);
    for (const tl of tails) {
      const end = Math.max(0, along - tl.gap);
      const seg = Math.min(tl.len * trail, end);
      tl.el.style.strokeDasharray = `${seg} ${total + tl.len}`;
      tl.el.style.strokeDashoffset = `${-(end - seg)}`;
    }
    running = true;
    requestAnimationFrame(frame);
  }
  function kick() { if (!running) requestAnimationFrame(frame); }

  addEventListener("scroll", () => {
    lastScroll = performance.now();
    if (playing && Math.abs(scrollY - playing.scroll) > 40) stop(); // a real scroll, not a layout nudge
    kick();
  }, { passive: true });
  addEventListener("resize", layout);
  new ResizeObserver(() => layout()).observe(document.body);
  document.fonts?.ready.then(layout);
  addEventListener("load", layout);
  lastScroll = performance.now();
  layout();
  // Birth: the hero's Create secret key button is pressed (CSS on .birth),
  // then a flash, rings and sparks, and the key spins into being.
  // The button stays in view a moment first, so the reader sees what gets pressed.
  setTimeout(() => {
    bornEl.classList.add("go");
    setTimeout(() => {
      root.classList.add("born");
      key.classList.add("alive");
    }, reduced ? 0 : 450);
  }, reduced ? 0 : 700);
}

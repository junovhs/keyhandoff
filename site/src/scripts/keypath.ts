// One key's journey down the homepage. A solid holographic key is born beside
// the hero buttons ([data-born]), then glides down the page on long swooping
// curves that cross from side to side, resting at each milestone ([data-stop],
// with data-label) and landing in the download logo. It trails a short comet
// tail; the path ahead is never drawn. Wider screens only (900px and up).
const SVG = "http://www.w3.org/2000/svg";
// The key from the KeyHandoff logo (Monolith, as Logo.astro draws it): a
// solid block with a square hole, its blade and teeth pointing down.
const KEY = `url("data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="28 8 64 104"><path fill-rule="evenodd" d="M28 8H92V60H72V78H90V90H72V98H90V112H48V60H28ZM50 24V44H70V24Z"/></svg>',
)}")`;
const TAILS = [["t1", 560, 120], ["t2", 440, 200], ["t3", 320, 290]] as const;

const stops = [...document.querySelectorAll<HTMLElement>("[data-stop]")];
const bornAt = document.querySelector<HTMLElement>("[data-born]");
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
if (stops.length > 1 && bornAt) init(bornAt);

function init(bornEl: HTMLElement) {
  const root = document.createElement("div");
  root.className = "keypath";
  root.setAttribute("aria-hidden", "true");
  root.style.setProperty("--key-mask", KEY);
  const sparks = Array.from({ length: 14 }, (_, i) => {
    const a = (i / 14) * Math.PI * 2, r = 110 + (i % 3) * 40;
    return `<i class="kp-spark" style="--sx:${Math.cos(a) * r}px;--sy:${Math.sin(a) * r}px"></i>`;
  }).join("");
  root.innerHTML =
    `<svg class="kp-svg"><defs><linearGradient id="kp-pearl" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="600" y2="600" spreadMethod="reflect"><stop offset="0" stop-color="#ddd5ff"/><stop offset=".33" stop-color="#c4ece6"/><stop offset=".66" stop-color="#f4d4e8"/><stop offset="1" stop-color="#d3cdff"/></linearGradient></defs>` +
    `<path class="kp-route" fill="none" stroke="none"/>${TAILS.map(([c]) => `<path class="kp-tail ${c}"/>`).join("")}</svg>` +
    `<div class="kp-burst"><i class="kp-flash"></i><i class="kp-ring"></i><i class="kp-ring r2"></i>${sparks}</div>` +
    `<div class="kp-key"><div class="kp-spin"><div class="kp-body"><i class="kp-shape"></i><i class="kp-glint"></i></div></div><span class="kp-label"></span></div>`;
  document.body.prepend(root);
  const svg = root.querySelector("svg")!;
  const route = root.querySelector<SVGPathElement>(".kp-route")!;
  const tails = TAILS.map(([c, len, gap]) => ({ el: root.querySelector<SVGPathElement>(`.kp-tail.${c}`)!, len, gap }));
  const burst = root.querySelector<HTMLElement>(".kp-burst")!;
  const key = root.querySelector<HTMLElement>(".kp-key")!;
  const label = root.querySelector<HTMLElement>(".kp-label")!;
  const spin = root.querySelector<HTMLElement>(".kp-spin")!;

  type Pt = { x: number; y: number };
  let pts: Pt[] = []; // birth, then one per stop
  let total = 0;
  let table: { len: number; y: number }[] = [];
  let lens: number[] = []; // path length at each point
  let wide = false;
  let shown = 0; // smoothed length
  let angle = 90;
  let current = -1;
  let labelTimer = 0;
  let trail = 0; // 0..1, how much tail shows; rises with speed, fades at rest
  let idle = 0; // 0..1, how much the resting float shows; eases in after a pause

  const docY = (el: HTMLElement) => el.getBoundingClientRect().top + scrollY;

  function layout() {
    wide = innerWidth >= 900;
    const docH = document.documentElement.scrollHeight;
    root.style.height = `${docH}px`;
    if (!wide) return;
    const wrapLeft = (document.querySelector(".wrap.hero") as HTMLElement).getBoundingClientRect().left;
    const left = Math.max(wrapLeft + 60, innerWidth * 0.2), right = innerWidth - left;
    const b = bornEl.getBoundingClientRect();
    pts = [{ x: b.right + 150, y: b.top + scrollY + b.height / 2 }];
    stops.forEach((s, i) => {
      if (i === stops.length - 1) {
        const r = s.getBoundingClientRect();
        pts.push({ x: r.left + r.width / 2, y: r.top + scrollY + r.height / 2 });
      } else if (i === 0) {
        // The first stop is the headline the key was born beside; rest just below the hero instead.
        const hero = document.querySelector(".wrap.hero") as HTMLElement;
        pts.push({ x: innerWidth * 0.5, y: docY(hero) + hero.offsetHeight + 40 });
      } else pts.push({ x: i % 2 ? left : right, y: docY(s) - 34 });
    });
    // One smooth spline through every point (Catmull-Rom as cubic Béziers),
    // with stretched tangents so each leg is a long, round swoop.
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
    // Length along the path at each point, leg by leg (the path may loop, so not by height).
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
    if (!shown) shown = target();
    kick();
  }

  /** Path length the key should be at: it rests at each point for the first
   *  third of the scroll towards the next, then travels, eased. */
  function target() {
    return keepInView(rest());
  }

  /** Never let the key leave the screen: if resting would leave it above or
   *  below the view, slide it along the path until it's back in. */
  function keepInView(len: number) {
    const top = scrollY + 200, bottom = scrollY + innerHeight - 120; // below the sticky nav
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

  function rest() {
    const anchor = scrollY + innerHeight * 0.5;
    if (anchor <= pts[0].y) return 0;
    for (let i = 0; i < pts.length - 1; i++) {
      const a = pts[i].y, b = pts[i + 1].y;
      if (anchor > b) continue;
      const t = (anchor - a) / (b - a), hold = 0.3;
      const m = t < hold ? 0 : (t - hold) / (1 - hold);
      const e = m < 0.5 ? 4 * m * m * m : 1 - (-2 * m + 2) ** 3 / 2;
      return lens[i] + (lens[i + 1] - lens[i]) * e;
    }
    return total;
  }

  let running = false;
  function frame() {
    if (!wide || !table.length) { running = false; return; }
    const goal = target();
    const step = reduced ? goal - shown : (goal - shown) * 0.09;
    shown += step;
    if (Math.abs(goal - shown) < 0.5) shown = goal;
    trail = reduced ? 0 : Math.max(Math.min(1, Math.abs(step) / 6), trail - 0.025);
    // While the reader pauses, the key floats like a fairy keeping them
    // company: a slow bob, a little sway and a slight tilt.
    const moving = Math.abs(step) > 0.3;
    idle = reduced ? 0 : moving ? Math.max(0, idle - 0.08) : Math.min(1, idle + 0.012);
    const t = performance.now() / 1000;
    const bob = Math.sin(t * 1.7) * 9 * idle, sway = Math.sin(t * 0.83 + 1) * 7 * idle, tilt = Math.sin(t * 1.1) * 7 * idle;
    const p = route.getPointAtLength(shown);
    const q = route.getPointAtLength(Math.min(total, shown + 2));
    const want = shown >= total - 1 ? angle : (Math.atan2(q.y - p.y, q.x - p.x) * 180) / Math.PI;
    let delta = want - angle;
    delta = ((delta + 540) % 360) - 180;
    angle += reduced ? delta : delta * 0.15;
    const landed = shown >= total - 2;
    // Home: the key melts into the download logo, which lights up.
    key.classList.toggle("landed", landed);
    stops[stops.length - 1].classList.toggle("kp-home", landed);
    key.style.transform = `translate(${p.x + sway}px, ${p.y + bob}px) scale(${landed ? 0.8 : 1})`;
    spin.style.transform = `rotate(${angle - 90 + tilt}deg)`; // the logo key points down
    for (const t of tails) {
      const end = Math.max(0, shown - t.gap);
      const seg = Math.min(t.len * trail, end);
      t.el.style.strokeDasharray = `${seg} ${total + t.len}`;
      t.el.style.strokeDashoffset = `${-(end - seg)}`;
    }
    // The milestone the key has reached shows its name for a moment.
    let i = 0;
    lens.forEach((l, n) => { if (shown >= l - 3) i = n; });
    if (i !== current) {
      current = i;
      const text = i === 0 ? "Born" : stops[i - 1].dataset.label ?? "";
      label.textContent = text;
      key.classList.add("labelled");
      clearTimeout(labelTimer);
      labelTimer = window.setTimeout(() => key.classList.remove("labelled"), 2200);
    }
    // Keep animating: moving, a fading tail, or floating at rest (stops when the tab is hidden).
    if (shown !== goal || trail > 0 || !reduced) {
      running = true;
      requestAnimationFrame(frame);
    } else running = false;
  }
  const kick = () => { if (!running) requestAnimationFrame(frame); };

  addEventListener("scroll", kick, { passive: true });
  addEventListener("resize", layout);
  new ResizeObserver(() => layout()).observe(document.body);
  document.fonts?.ready.then(layout);
  addEventListener("load", layout);
  layout();
  // Birth: flash, rings and sparks, then the key spins into being.
  setTimeout(() => {
    root.classList.add("born");
    key.classList.add("alive");
  }, reduced ? 0 : 500);
}

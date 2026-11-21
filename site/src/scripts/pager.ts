// The homepage turns like pages. Each section fills the screen; a scroll past a
// small threshold slides to the next one, and anything less only bends the page
// a little and springs back. Wide screens with a mouse or trackpad only: phones,
// short windows and reduced motion keep ordinary scrolling.

const NAV = 96;
const THRESHOLD = 70; // px of wheel travel before the page turns
const BEND = 0.18; // how far the page follows a scroll below the threshold
const SLIDE_MS = 700;

const pages = [...document.querySelectorAll<HTMLElement>("header.hero, section.wrap")];
const fits = () => innerWidth >= 900 && innerHeight >= 620 && !matchMedia("(prefers-reduced-motion: reduce)").matches;

if (pages.length > 1) {
  const root = document.documentElement;
  let pull = 0, moving = false, quietTimer = 0, lastWheel = 0, settleTimer = 0;

  const tops = () => [...pages.map((p) => Math.max(0, p.getBoundingClientRect().top + scrollY - NAV)), document.documentElement.scrollHeight - innerHeight];
  /** The page the reader is on: the last one whose top is at or above the screen top. */
  const current = () => {
    const t = tops();
    let i = 0;
    for (let k = 0; k < t.length; k++) if (t[k] <= scrollY + 4) i = k;
    return i;
  };

  const bend = (px: number) => {
    root.style.setProperty("--bend", `${-px}px`);
  };
  const spring = () => {
    pull = 0;
    root.classList.add("springing");
    bend(0);
    setTimeout(() => root.classList.remove("springing"), 360);
  };

  const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
  const slideTo = (y: number) => {
    moving = true;
    pull = 0;
    root.classList.remove("springing");
    bend(0);
    const from = scrollY, d = y - from, t0 = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, (now - t0) / SLIDE_MS);
      scrollTo({ top: from + d * ease(t), behavior: "instant" });
      if (t < 1) requestAnimationFrame(step);
      else setTimeout(() => (moving = false), 120);
    };
    requestAnimationFrame(step);
  };

  /** Turn one page in direction dir, or scroll within a page taller than the screen. */
  const turn = (dir: 1 | -1) => {
    const t = tops(), i = current();
    const page = pages[i];
    if (page && dir > 0) {
      const bottom = page.getBoundingClientRect().bottom;
      if (bottom > innerHeight + 8) return slideTo(Math.min(scrollY + innerHeight * 0.8, scrollY + bottom - innerHeight + 24));
    }
    if (dir < 0 && scrollY > t[i] + 4) return slideTo(t[i]);
    const next = Math.max(0, Math.min(t.length - 1, i + dir));
    if (next !== i) slideTo(t[next]);
  };

  addEventListener("wheel", (e) => {
    if (!fits() || e.ctrlKey) return;
    e.preventDefault();
    const now = performance.now();
    const gap = now - lastWheel;
    lastWheel = now;
    // A trackpad keeps sending a tail of small events after a fling; wait for
    // a pause before the next turn so one swipe turns one page.
    if (moving) return;
    if (gap > 250) pull = 0;
    const dy = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaY;
    pull += dy;
    clearTimeout(quietTimer);
    if (Math.abs(pull) >= THRESHOLD) {
      const dir = pull > 0 ? 1 : -1;
      turn(dir);
      return;
    }
    bend(pull * BEND);
    quietTimer = window.setTimeout(spring, 140);
  }, { passive: false });

  addEventListener("keydown", (e) => {
    if (!fits() || e.defaultPrevented || e.altKey || e.ctrlKey || e.metaKey) return;
    const t = e.target as HTMLElement;
    if (t.closest("input, textarea, select, [contenteditable]")) return;
    const down = ["PageDown", "ArrowDown", " "].includes(e.key) && !(e.key === " " && e.shiftKey);
    const up = ["PageUp", "ArrowUp"].includes(e.key) || (e.key === " " && e.shiftKey);
    if (e.key === "Home") { e.preventDefault(); slideTo(0); }
    else if (e.key === "End") { e.preventDefault(); slideTo(tops().at(-1)!); }
    else if (down || up) { e.preventDefault(); if (!moving) turn(down ? 1 : -1); }
  });

  // A scrollbar drag or a jump link can leave the page between sections;
  // when the reader lets go, settle onto the nearest one.
  addEventListener("scroll", () => {
    if (moving || !fits()) return;
    clearTimeout(settleTimer);
    settleTimer = window.setTimeout(() => {
      if (moving || performance.now() - lastWheel < 400) return;
      const t = tops(), i = current(), page = pages[i];
      if (page && page.getBoundingClientRect().bottom > innerHeight + 8 && scrollY >= t[i]) return; // reading inside a tall page
      const near = t.reduce((a, b) => (Math.abs(b - scrollY) < Math.abs(a - scrollY) ? b : a));
      if (Math.abs(near - scrollY) > 4) slideTo(near);
    }, 300);
  }, { passive: true });
}

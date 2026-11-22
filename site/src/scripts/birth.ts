// How a key is born safe, under the 29 million. The finished picture is the
// resting state. Each time it comes into view this replays it: a cursor clicks
// Create secret key and the key is born out of the new value; Copy; Safe Copy
// in the real window catches the one copy (it had already checked nothing was
// watching) and wipes the clipboard; the key flies into the vault. It loops.
// While it plays, the page's travelling key (keypath.ts) waits hidden at the vault.

const birth = document.querySelector<HTMLElement>("[data-birth]");
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;

if (birth && !reduced) {
  const key = birth.querySelector<HTMLElement>(".bt-key")!;
  const cursor = birth.querySelector<SVGElement>(".bt-cursor")!;
  const q = (sel: string) => birth.querySelector<HTMLElement>(sel)!;
  const at = (el: Element, fx = 0.5, fy = 0.5) => {
    const a = birth.getBoundingClientRect(), r = el.getBoundingClientRect();
    return `translate(${r.left - a.left + r.width * fx}px, ${r.top - a.top + r.height * fy}px)`;
  };
  const step = (n: number) => { for (let i = 1; i <= 5; i++) birth.classList.toggle(`k${i}`, i <= n); };
  const click = (el: HTMLElement) => {
    cursor.style.transform = at(el, 0.55, 0.6);
    cursor.classList.add("show");
    later(600, () => { cursor.classList.add("press"); el.classList.add("bt-press"); });
    later(860, () => { cursor.classList.remove("press"); el.classList.remove("bt-press"); });
  };
  let timers: number[] = [];
  const later = (ms: number, f: () => void) => timers.push(window.setTimeout(f, ms));
  const clear = () => { timers.forEach(clearTimeout); timers = []; };

  const play = () => {
    clear();
    document.documentElement.classList.add("birth-hold");
    birth.classList.add("bt-run");
    birth.dataset.bw = "listen"; // Safe Copy is armed first, and has checked nothing is watching
    step(0);
    key.className = "bt-key";
    key.style.transition = "none";
    cursor.style.transition = "none";
    cursor.style.transform = at(q("[data-bt-create]"), 0.6, 2.6);
    void key.offsetWidth;
    key.style.transition = cursor.style.transition = "";
    // The key lives in the space between the page and the window, and barely moves.
    const home = q("[data-born-at]");
    key.style.transform = at(home);
    // Create secret key: the value appears and the key is born beside it.
    later(200, () => click(q("[data-bt-create]")));
    later(900, () => { step(2); key.classList.add("show", "pop"); });
    // Copy: the key slips into the window, Safe Copy catches it and wipes the clipboard.
    later(1500, () => click(q("[data-bt-copy]")));
    later(2150, () => { key.style.transform = at(q(".bt-win"), 0.02, 0.5); key.classList.add("in"); cursor.classList.remove("show"); });
    later(2600, () => { birth.dataset.bw = "caught"; step(3); });
    // Into the vault: the new row slides in, sealed, and the shield lights.
    later(4000, () => { birth.dataset.bw = ""; });
    later(4300, () => step(5));
    // The key steps back out beside the window, at rest.
    later(5200, () => { key.classList.remove("in"); key.style.transform = at(home); });
    // Rest on the finished picture, the key beside the vault, then play again.
    later(6200, () => { birth.classList.remove("bt-run"); step(0); });
    later(9600, () => { key.classList.remove("show"); later(300, play); });
  };

  // As soon as any of it shows, the travelling key steps aside for this one, so
  // the replay never opens with a key already in the vault. It loops while in view;
  // once the section is gone the travelling key takes over from the vault.
  let played = false;
  new IntersectionObserver(([e]) => {
    if (e.intersectionRatio < 0.15) {
      clear(); played = false;
      birth.classList.remove("bt-run"); birth.dataset.bw = ""; step(0); key.className = "bt-key"; cursor.classList.remove("show");
      document.documentElement.classList.remove("birth-hold");
      return;
    }
    if (!played) document.documentElement.classList.add("birth-hold");
    if (!played && e.intersectionRatio >= 0.5) { played = true; play(); }
  }, { threshold: [0, 0.15, 0.5], rootMargin: "-96px 0px 0px 0px" }).observe(birth);
}

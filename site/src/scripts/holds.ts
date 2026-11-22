// Sections that play their own key ([data-hold-key]) ask the page's travelling
// key (keypath.ts) to step aside while they're on screen, so there's one key.
const holders = [...document.querySelectorAll<HTMLElement>("[data-hold-key]")];
const seen = new Set<Element>();
if (holders.length) {
  const io = new IntersectionObserver((entries) => {
    for (const e of entries) e.intersectionRatio > 0.25 ? seen.add(e.target) : seen.delete(e.target);
    document.documentElement.classList.toggle("key-hold", seen.size > 0);
  }, { threshold: [0, 0.25] });
  holders.forEach((h) => io.observe(h));
}

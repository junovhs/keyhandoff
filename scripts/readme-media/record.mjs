// Records the README's banner and GIFs from the built site (site/dist) to docs/readme/.
// Run from the repo root after `npm run build` in site/: node scripts/readme-media/record.mjs
// Needs ffmpeg and a Chromium (set SHOT_BROWSER if it isn't at /usr/bin/chromium).
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as wait } from "node:timers/promises";

const SITE_PORT = 4390;
const DEBUG_PORT = 9334;
const out = resolve("docs/readme");
// [file, element, seconds to record, widest crop in page pixels]
const CLIPS = [
  ["hero", "#scan", 10.5, 760],
  ["keep", "#solo-stage", 7.6, 980],
  ["send", "#duo", 21.8, 880],
  ["teams", "#circle-stage", 17.6, 860],
];

mkdirSync(out, { recursive: true });
const temp = mkdtempSync(join(tmpdir(), "keyhandoff-readme-"));
const site = spawn("python3", ["-m", "http.server", String(SITE_PORT), "-d", resolve("site/dist")], { stdio: "ignore" });
const chrome = spawn(process.env.SHOT_BROWSER ?? "/usr/bin/chromium",
  ["--headless=new", `--remote-debugging-port=${DEBUG_PORT}`, `--user-data-dir=${join(temp, "profile")}`, "--hide-scrollbars", "about:blank"], { stdio: "ignore" });

async function endpoint(path, method = "GET") {
  for (let tries = 0; tries < 50; tries++) {
    try { return await (await fetch(`http://127.0.0.1:${DEBUG_PORT}${path}`, { method })).json(); } catch { await wait(200); }
  }
  throw new Error("The browser's debugging port never opened.");
}
const page = await endpoint("/json/new?about:blank", "PUT");
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((open) => socket.addEventListener("open", open));
let nextId = 0;
const pending = new Map();
socket.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  pending.get(msg.id)?.(msg.result);
  pending.delete(msg.id);
});
const send = (method, params = {}) => new Promise((done) => {
  pending.set(++nextId, done);
  socket.send(JSON.stringify({ id: nextId, method, params }));
});
const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result.value;

try {
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });

  // The banner: the logo and the line, in the site's own fonts and colours.
  const fonts = resolve("site/public/fonts");
  writeFileSync(join(temp, "banner.html"), readFileSync(resolve("scripts/readme-media/banner.html"), "utf8").replaceAll("FONTS", `file://${fonts}`));
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 400, deviceScaleFactor: 2, mobile: false });
  await send("Page.navigate", { url: `file://${join(temp, "banner.html")}` });
  await wait(1200);
  const banner = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(out, "banner.png"), Buffer.from(banner.data, "base64"));
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });

  for (const [file, selector, seconds, widest] of CLIPS) {
    await send("Page.navigate", { url: `http://127.0.0.1:${SITE_PORT}/` });
    await wait(1500);
    await evaluate(`document.querySelector("nav").style.display = "none"`);
    // Bring the element to the middle of the screen so it fades in, then start its animation over
    // from the top: the hero by its Replay button, the others by leaving the screen and coming back.
    const centre = `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); window.scrollTo(0, scrollY + r.top - (innerHeight - r.height) / 2); })()`;
    await evaluate(centre);
    await wait(2000);
    if (file === "hero") await evaluate(`document.getElementById("scan-again").click()`);
    else {
      await evaluate(`window.scrollTo(0, scrollY + 3 * innerHeight)`);
      await wait(400);
      await evaluate(centre);
    }
    // Captures come back blank for a moment after a scroll, so give the page time to paint.
    await wait(700);
    const box = await evaluate(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); const w = Math.min(r.width + 48, ${widest}); return { x: r.left + scrollX + (r.width - w) / 2, y: r.top + scrollY - 24, width: w, height: r.height + 48 }; })()`);
    const frames = join(temp, file);
    mkdirSync(frames);
    const list = [];
    const start = Date.now();
    let last = start;
    for (let i = 0; Date.now() - start < seconds * 1000; i++) {
      const { data } = await send("Page.captureScreenshot", { format: "jpeg", quality: 92, clip: { ...box, scale: 1 }, captureBeyondViewport: false });
      const name = `${String(i).padStart(5, "0")}.jpg`;
      writeFileSync(join(frames, name), Buffer.from(data, "base64"));
      const now = Date.now();
      if (list.length) list[list.length - 1].d = (now - last) / 1000;
      list.push({ name, d: 0.08 });
      last = now;
      await wait(Math.max(0, 60 - (Date.now() - now)));
    }
    writeFileSync(join(frames, "list.txt"), list.map((f) => `file '${f.name}'\nduration ${f.d.toFixed(3)}`).join("\n") + `\nfile '${list.at(-1).name}'\n`);
    const width = Math.round(box.width);
    execFileSync("ffmpeg", ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", join(frames, "list.txt"),
      "-vf", `fps=15,scale=${width}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle`,
      join(out, `${file}.gif`)]);
    console.log(`${file}.gif: ${list.length} frames`);
  }
} finally {
  socket.close();
  chrome.kill();
  site.kill();
  await wait(500);
  rmSync(temp, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}

// Renders the app's real screens with sample data to site/public/shots/*.png.
// Run from the repo root: node scripts/site-shots/shoot.mjs
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as wait } from "node:timers/promises";
import { build, preview } from "vite";

const WIDTH = 400;
const HEIGHT = 600;
const SETTLE_MS = 1500; // long enough for the screens' entrance motion to finish
const DEBUG_PORT = 9333;
const SCENES = ["home", "ask", "listening", "sealed", "accepted"];
const BROWSERS = [
  process.env.SHOT_BROWSER,
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
];

const browser = BROWSERS.find((b) => b && existsSync(b));
if (!browser) throw new Error("No Chromium browser found. Set SHOT_BROWSER.");

const temp = mkdtempSync(join(tmpdir(), "keyhandoff-shots-"));
const configFile = resolve("scripts/site-shots/vite.config.ts");
const outDir = join(temp, "build");
const out = resolve("site/public/shots");
mkdirSync(out, { recursive: true });

await build({ configFile, logLevel: "warn", build: { outDir, emptyOutDir: true } });
const server = await preview({ configFile, build: { outDir } });
const url = server.resolvedUrls.local[0];

// Headless Chromium won't go narrower than 500px by flags, so size it over its
// debugging protocol, which also gives the screens time to settle.
const chrome = spawn(browser, ["--headless=new", `--remote-debugging-port=${DEBUG_PORT}`, `--user-data-dir=${join(temp, "profile")}`, "--hide-scrollbars", "about:blank"], { stdio: "ignore" });

async function debugEndpoint(path, method = "GET") {
  for (let tries = 0; tries < 50; tries++) {
    try {
      return await (await fetch(`http://127.0.0.1:${DEBUG_PORT}${path}`, { method })).json();
    } catch {
      await wait(200);
    }
  }
  throw new Error("The browser's debugging port never opened.");
}

const page = await debugEndpoint("/json/new?about:blank", "PUT");
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((open) => socket.addEventListener("open", open));

let nextId = 0;
const pending = new Map();
socket.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  pending.get(msg.id)?.(msg.result);
  pending.delete(msg.id);
});
const send = (method, params = {}) =>
  new Promise((done) => {
    pending.set(++nextId, done);
    socket.send(JSON.stringify({ id: nextId, method, params }));
  });

await send("Emulation.setDeviceMetricsOverride", { width: WIDTH, height: HEIGHT, deviceScaleFactor: 2, mobile: false });
for (const scene of SCENES) {
  await send("Page.navigate", { url: `${url}?s=${scene}` });
  await wait(SETTLE_MS);
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(resolve(out, `${scene}.png`), Buffer.from(data, "base64"));
  console.log(`${scene}.png`);
}

socket.close();
chrome.kill();
await server.close();
await wait(500); // the browser lets go of its profile
rmSync(temp, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
process.exit(0);

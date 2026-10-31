// Builds the mocked screens, serves them, and drives headless Chromium over its
// debugging protocol. Shared by shoot.mjs (site screenshots) and board.mjs.
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as wait } from "node:timers/promises";
import { build, preview } from "vite";

const DEBUG_PORT = 9333;
const BROWSERS = [
  process.env.SHOT_BROWSER,
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
];

/** Run `work({ url, send })` against a served build; `send` is a CDP call. */
export async function withBrowser(work) {
  const browser = BROWSERS.find((b) => b && existsSync(b));
  if (!browser) throw new Error("No Chromium browser found. Set SHOT_BROWSER.");
  const temp = mkdtempSync(join(tmpdir(), "keyhandoff-shots-"));
  const configFile = resolve("scripts/site-shots/vite.config.ts");
  const outDir = join(temp, "build");
  await build({ configFile, logLevel: "warn", build: { outDir, emptyOutDir: true } });
  const server = await preview({ configFile, build: { outDir } });
  const url = server.resolvedUrls.local[0];
  // Headless Chromium won't go narrower than 500px by flags, so pages are sized
  // over the debugging protocol instead.
  const chrome = spawn(browser, ["--headless=new", `--remote-debugging-port=${DEBUG_PORT}`, `--user-data-dir=${join(temp, "profile")}`, "--hide-scrollbars", "about:blank"], { stdio: "ignore" });

  async function endpoint(path, method = "GET") {
    for (let tries = 0; tries < 50; tries++) {
      try {
        return await (await fetch(`http://127.0.0.1:${DEBUG_PORT}${path}`, { method })).json();
      } catch {
        await wait(200);
      }
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
  const send = (method, params = {}) =>
    new Promise((done) => {
      pending.set(++nextId, done);
      socket.send(JSON.stringify({ id: nextId, method, params }));
    });

  try {
    await work({ url, send });
  } finally {
    socket.close();
    chrome.kill();
    await server.close();
    await wait(500); // the browser lets go of its profile
    rmSync(temp, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
}

// Renders the app's real screens with sample data to site/public/shots/*.png.
// Run from the repo root: node scripts/site-shots/shoot.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as wait } from "node:timers/promises";
import { withBrowser } from "./chrome.mjs";

const WIDTH = 400;
const HEIGHT = 600;
const SETTLE_MS = 1500; // long enough for the screens' entrance motion to finish
const SCENES = ["home", "card", "listening", "sealed", "accepted", "locked-pin"];

const out = resolve("site/public/shots");
mkdirSync(out, { recursive: true });
await withBrowser(async ({ url, send }) => {
  await send("Emulation.setDeviceMetricsOverride", { width: WIDTH, height: HEIGHT, deviceScaleFactor: 2, mobile: false });
  for (const scene of SCENES) {
    await send("Page.navigate", { url: `${url}?s=${scene}` });
    await wait(SETTLE_MS);
    const { data } = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(resolve(out, `${scene}.png`), Buffer.from(data, "base64"));
    console.log(`${scene}.png`);
  }
});
process.exit(0);

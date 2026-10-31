// Renders every app screen, laid out in flow order, to docs/screens.png.
// Run from the repo root: node scripts/site-shots/board.mjs
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as wait } from "node:timers/promises";
import { withBrowser } from "./chrome.mjs";

const SETTLE_MS = 2500; // every screen's entrance motion has finished

await withBrowser(async ({ url, send }) => {
  await send("Emulation.setDeviceMetricsOverride", { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `${url}board.html` });
  await wait(SETTLE_MS);
  const { result } = await send("Runtime.evaluate", { expression: "JSON.stringify([document.body.scrollWidth, document.body.scrollHeight])", returnByValue: true });
  const [width, height] = JSON.parse(result.value);
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  await wait(SETTLE_MS);
  const { data } = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true });
  writeFileSync(resolve("docs/screens.png"), Buffer.from(data, "base64"));
  console.log(`docs/screens.png ${width}x${height}`);
});
process.exit(0);

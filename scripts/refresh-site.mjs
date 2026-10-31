// After a release is published: trigger the Cloudflare build of keyhandoff.dev
// through its deploy hook, then wait until the live site serves the new
// release's download links. Run by .github/workflows/refresh-site.yml.
import { pathToFileURL } from "node:url";
import { appendFileSync } from "node:fs";
import { getLatestRelease } from "../site/src/lib/releases.mjs";

const HOOK_PATTERN = /^https:\/\/api\.cloudflare\.com\/client\/v4\/workers\/builds\/deploy_hooks\/[a-f\d-]{36}$/i;
const BASE = "https://keyhandoff.dev";

export async function refreshSite({ release, hookURL, fetchImpl = fetch,
  sleep = (ms) => new Promise((done) => setTimeout(done, ms)), attempts = 60 }) {
  if (!HOOK_PATTERN.test(hookURL ?? "")) throw new Error("CLOUDFLARE_SITE_DEPLOY_HOOK is missing or invalid");
  if (!release?.tag || release.packages.length !== 3) throw new Error("A complete published release is required");
  async function manifest() {
    const response = await fetchImpl(`${BASE}/release.json?t=${Date.now()}`, {
      headers: { "Cache-Control": "no-cache" }, signal: AbortSignal.timeout(20000),
    });
    if (response.status === 404) return null;
    if (!response.ok) throw new Error(`Website manifest returned HTTP ${response.status}`);
    return response.json();
  }
  const before = await manifest();
  const response = await fetchImpl(hookURL, { method: "POST", signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`Cloudflare build trigger returned HTTP ${response.status}`);
  const result = await response.json();
  if (!result.success || !/^[a-f\d-]{36}$/i.test(result.result?.build_uuid ?? "")) {
    throw new Error("Cloudflare did not accept the build trigger");
  }
  const buildUUID = result.result.build_uuid;
  for (let attempt = 0; attempt < attempts; attempt++) {
    await sleep(10000);
    let after;
    try { after = await manifest(); } catch { continue; }
    if (!after?.buildId || after.buildId === before?.buildId || after.tag !== release.tag) continue;
    const matches = release.packages.every((pkg) => after.packages?.some(
      (asset) => asset.name === pkg.name && asset.sha256 === pkg.sha256 && asset.url === pkg.url));
    if (!matches) continue;
    const page = await fetchImpl(`${BASE}/?build=${encodeURIComponent(after.buildId)}`, {
      headers: { "Cache-Control": "no-cache" }, signal: AbortSignal.timeout(20000),
    });
    if (!page.ok) continue;
    const html = await page.text();
    if (!html.includes(`data-release-tag="${release.tag}"`) || !release.packages.every((pkg) => html.includes(pkg.url))) continue;
    return { buildUUID, manifest: after };
  }
  throw new Error(`Website did not publish a fresh build for ${release.tag} within the verification window (build ${buildUUID})`);
}

async function main() {
  const hookURL = process.env.CLOUDFLARE_SITE_DEPLOY_HOOK;
  if (!HOOK_PATTERN.test(hookURL ?? "")) throw new Error("CLOUDFLARE_SITE_DEPLOY_HOOK is missing or invalid");
  const release = await getLatestRelease();
  if (process.env.RELEASE_TAG && release?.tag !== process.env.RELEASE_TAG) {
    throw new Error("Requested release is not the latest published release");
  }
  const result = await refreshSite({ release, hookURL });
  const message = `Website refreshed for ${release.tag}; Cloudflare build ${result.buildUUID}; live commit ${result.manifest.commit}`;
  console.log(message);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${message}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    // Fetch failures can contain the hook URL, which is a credential; never print raw errors.
    const safe = String(error.message).replace(/https?:\/\/\S+/g, "[redacted URL]");
    console.error(`Website refresh failed: ${safe}`);
    process.exitCode = 1;
  });
}

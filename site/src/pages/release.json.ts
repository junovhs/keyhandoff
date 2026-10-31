import { randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { getLatestRelease } from "../lib/releases.mjs";

// A fresh build id each build: CI checks the live site rebuilt after a release,
// even when the commit didn't change.
export const prerender = true;
export async function GET() {
  const release = await getLatestRelease();
  return new Response(
    JSON.stringify({
      buildId: randomUUID(),
      commit: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
      tag: release?.tag ?? null,
      packages: release?.packages.map(({ name, url, sha256 }) => ({ name, url, sha256 })) ?? [],
    }),
    { headers: { "Content-Type": "application/json" } },
  );
}

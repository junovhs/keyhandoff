// The download links come from the latest published GitHub Release and its own
// SHA256SUMS, read when the site builds. CI rebuilds the site after each release.
import { readFileSync } from "node:fs";

export const REPOSITORY = "junovhs/keyhandoff";
export const ALL_RELEASES = `https://github.com/${REPOSITORY}/releases`;

/** What each release carries, in the order the page shows them. */
const PACKAGES = [
  { id: "windows", os: "Windows", label: "Windows installer", suffix: "windows-x64-setup.exe" },
  { id: "deb", os: "Linux", label: "Linux .deb", suffix: "linux-amd64.deb" },
  { id: "appimage", os: "Linux", label: "Linux AppImage", suffix: "linux-x86_64.AppImage" },
];

async function request(url, accept) {
  const headers = { Accept: accept, "User-Agent": "keyhandoff-site" };
  if (new URL(url).hostname === "api.github.com" && process.env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  }
  return fetch(url, { headers, signal: AbortSignal.timeout(30000) });
}

export function parseChecksums(text) {
  return new Map(text.trim().split(/\r?\n/).map((line) => {
    const match = /^([a-f\d]{64})\s+\*?([^/\s]+)$/i.exec(line);
    if (!match) throw new Error("Malformed release SHA256SUMS");
    return [match[2], match[1].toLowerCase()];
  }));
}

function assetURL(asset) {
  const url = new URL(asset.browser_download_url);
  if (url.origin !== "https://github.com" || !url.pathname.startsWith(`/${REPOSITORY}/releases/download/`)) {
    throw new Error("Release asset URL is outside this repository");
  }
  return url.href;
}

let latest;
/** The latest release, or null before the first one. Fails the build on a broken release. */
export function getLatestRelease() {
  return (latest ??= loadRelease());
}

async function loadRelease() {
  let release, checksumText;
  if (process.env.KH_RELEASE_FIXTURE) {
    ({ release, checksums: checksumText } = JSON.parse(readFileSync(process.env.KH_RELEASE_FIXTURE, "utf8")));
    if (release === null) return null;
  } else {
    const response = await request(`https://api.github.com/repos/${REPOSITORY}/releases/latest`, "application/vnd.github+json");
    if (response.status === 404) return null;
    if (!response.ok) throw new Error(`GitHub latest release: HTTP ${response.status}`);
    release = await response.json();
  }
  if (!/^v\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/.test(release.tag_name) || release.draft || release.prerelease) {
    throw new Error("Expected a published versioned release");
  }
  const checksumsAsset = release.assets.find((a) => a.name === "SHA256SUMS");
  if (!checksumsAsset) throw new Error("Published release is missing SHA256SUMS");
  const checksumsURL = assetURL(checksumsAsset);
  if (checksumText === undefined) {
    const response = await request(checksumsURL, "application/octet-stream");
    if (!response.ok) throw new Error(`Release SHA256SUMS: HTTP ${response.status}`);
    checksumText = await response.text();
  }
  const sums = parseChecksums(checksumText);
  const version = release.tag_name.slice(1);
  const packages = PACKAGES.map((platform) => {
    const name = `keyhandoff-${version}-${platform.suffix}`;
    const asset = release.assets.find((a) => a.name === name && a.state === "uploaded");
    if (!asset || !Number.isSafeInteger(asset.size) || asset.size <= 0 || !sums.has(name)) {
      throw new Error(`Published release is missing a package or its checksum: ${name}`);
    }
    return { ...platform, name, url: assetURL(asset), size: asset.size, sha256: sums.get(name) };
  });
  if (!Number.isFinite(Date.parse(release.published_at))) throw new Error("Release has no publication date");
  return {
    tag: release.tag_name,
    version,
    date: release.published_at.slice(0, 10),
    url: `${ALL_RELEASES}/tag/${encodeURIComponent(release.tag_name)}`,
    checksumsURL,
    packages,
  };
}

export function formatSize(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

#!/usr/bin/env bash
# Builds keyhandoff.dev from main and deploys it to Cloudflare from the
# owner's computer, then waits until the live site serves this build.
#
# The deploy runs here, not in GitHub Actions, so the repository holds no
# Cloudflare credential: taking over the GitHub account can't change the
# site's download links any more than it can sign an update (ADR 0005).
# Wrangler uses the owner's own login (`npx wrangler login`, once).
#
#   scripts/deploy-site.sh
#
# Run it after site changes land on main, and after publishing a release
# (scripts/sign-release.sh runs it) so the download links follow.
set -euo pipefail

die() { echo "deploy-site: $*" >&2; exit 1; }
cd "$(git rev-parse --show-toplevel)"

git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "check out origin/main first (git switch main && git pull --ff-only)"
git diff --quiet HEAD -- site src || die "site/ or src/ has uncommitted changes; the site must match main"
commit=$(git rev-parse HEAD)

# The site builds with Node 24, as the site workflow does.
if [ "$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)" -lt 24 ] && [ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]; then
  set +u; . "${NVM_DIR:-$HOME/.nvm}/nvm.sh"; nvm use -s 24 >/dev/null || true; set -u
fi
[ "$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)" -ge 24 ] || die "needs Node.js 24 or newer"

cd site
npm ci --silent
# The download section reads the latest release from GitHub at build time.
GITHUB_TOKEN="${GITHUB_TOKEN:-$(gh auth token)}" npm run build --silent
npx wrangler deploy

echo "Waiting for keyhandoff.dev to serve $commit"
for _ in $(seq 1 30); do
  live=$(curl -fsS -H 'Cache-Control: no-cache' "https://keyhandoff.dev/release.json?t=$(date +%s%N)" || true)
  if printf '%s' "$live" | grep -q "\"commit\":\"$commit\""; then
    tag=$(printf '%s' "$live" | sed -n 's/.*"tag":"\([^"]*\)".*/\1/p')
    echo "keyhandoff.dev is live at $commit, download links for ${tag:-no release}."
    exit 0
  fi
  sleep 5
done
die "deployed, but keyhandoff.dev still doesn't serve $commit after 150 seconds"

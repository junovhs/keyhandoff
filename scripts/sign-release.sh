#!/usr/bin/env bash
# Sign a drafted release on the owner's own machine, then publish it.
# CI builds and drafts releases without any signing key (ADR 0005); this is the
# only place the updater key is used. Steps are in docs/releasing.md.
#
# Usage: TAURI_SIGNING_PRIVATE_KEY_PATH=/path/on/signing/drive/keyhandoff.key \
#          scripts/sign-release.sh VERSION
# The key's password is asked for once, without echo, unless
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD is set. REPO=owner/name overrides the repository.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "sign-release: $*" >&2; exit 1; }

v="${1:?usage: scripts/sign-release.sh VERSION}"
repo="${REPO:-junovhs/keyhandoff}"
key="${TAURI_SIGNING_PRIVATE_KEY_PATH:-}"
[ -n "$key" ] || die "set TAURI_SIGNING_PRIVATE_KEY_PATH to the updater key file on the signing drive"
[ -s "$key" ] || die "no key file at $key (is the signing drive plugged in and unlocked?)"
[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || die "TAURI_SIGNING_PRIVATE_KEY is set; unset it and use the key file only"
# A shell that hasn't loaded nvm has no node; load it if it's there.
if ! command -v node >/dev/null && [ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]; then
  set +u; . "${NVM_DIR:-$HOME/.nvm}/nvm.sh"; nvm use -s 24 >/dev/null 2>&1 || nvm use -s default >/dev/null 2>&1 || true; set -u
fi
for tool in gh git jq node npx sha256sum; do
  command -v "$tool" >/dev/null || die "$tool is not installed"
done

# 1. The draft: it must exist, still be a draft, and target a commit on main
#    whose version is the one being signed.
[ "$(gh release view "v$v" -R "$repo" --json isDraft --jq .isDraft)" = true ] \
  || die "v$v is not a draft release (already published, or never drafted)"
target=$(gh release view "v$v" -R "$repo" --json targetCommitish --jq .targetCommitish)
git fetch -q origin main
git cat-file -e "$target^{commit}" 2>/dev/null || die "draft targets $target, which isn't in this checkout"
git merge-base --is-ancestor "$target" origin/main || die "draft targets $target, which is not on main"
built=$(git show "$target:src-tauri/Cargo.toml" | sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' | head -1)
[ "$built" = "$v" ] || die "draft targets $target, whose version is $built, not $v"
echo "Signing v$v, built from $(git log -1 --format='%h %s' "$target")"

# 2. Download the packages and check them against the hashes CI printed.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
gh release download "v$v" -R "$repo" -D "$work" -p 'keyhandoff-*' -p SHA256SUMS
(cd "$work" && sha256sum -c --strict SHA256SUMS) || die "a package doesn't match SHA256SUMS"
echo "Compare these with the 'draft' step of the release run's log before going on:"
cat "$work/SHA256SUMS"

# 3. Sign each package, then check every signature against the key installed
#    copies trust, so a wrong key file is caught before anything is uploaded.
# The .dmg is only a download; installed Macs update from the .app.tar.gz.
packages=("keyhandoff-$v-windows-x64-setup.exe" "keyhandoff-$v-linux-x86_64.AppImage" "keyhandoff-$v-linux-amd64.deb" "keyhandoff-$v-macos-universal.app.tar.gz")
# tauri signer never prompts: without a password it assumes an empty one.
password="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD-}"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+set}" ]; then
  read -r -s -p "Password for $key: " password
  echo
fi
for p in "${packages[@]}"; do
  [ -s "$work/$p" ] || die "the draft has no $p"
  rm -f "$work/$p.sig"
  echo "Signing $p"
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$password" npx tauri signer sign -f "$key" "$work/$p" >/dev/null \
    || die "signing $p failed (wrong password?)"
  [ -s "$work/$p.sig" ] || die "signing $p produced no signature"
done
node scripts/verify-update-sig.mjs "${packages[@]/#/$work/}" \
  || die "a signature doesn't verify against tauri.conf.json's updater key; wrong key file?"

# 4. The update manifest, with the draft's own notes, and the upload.
gh release view "v$v" -R "$repo" --json body --jq .body > "$work/notes.md"
bash scripts/updater-manifest.sh "$v" "$work" "$work/notes.md" > "$work/latest.json"
gh release upload "v$v" -R "$repo" --clobber "$work"/*.sig "$work/latest.json"
echo "Signatures and latest.json are on the draft."

# 5. Publishing makes the tag and reaches installed copies; then the site is
# deployed from here so its download links follow.
read -r -p "Publish v$v to everyone now? [y/N] " answer
if [ "$answer" != y ] && [ "$answer" != Y ]; then
  echo "Left as a signed draft. Publish later with: gh release edit v$v -R $repo --draft=false --latest"
  exit 0
fi
gh release edit "v$v" -R "$repo" --draft=false --latest >/dev/null
echo "Published: https://github.com/$repo/releases/tag/v$v"
bash scripts/deploy-site.sh || echo "The site didn't update; run scripts/deploy-site.sh again from main."

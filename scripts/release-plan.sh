#!/usr/bin/env bash
# Release gate for release.yml's plan job: prints `version=X` and
# `release=true|false` (GITHUB_OUTPUT lines). The app's version is
# src-tauri/Cargo.toml's; a version with no vX.Y.Z tag on origin is released, and
# fails here if CHANGELOG.md has no notes for it. NOTES=FILE keeps the notes.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' src-tauri/Cargo.toml | head -1)
[ -n "$version" ] || { echo "no [package] version in src-tauri/Cargo.toml" >&2; exit 1; }
echo "version=$version"
if git ls-remote --exit-code --tags origin "refs/tags/v$version" >/dev/null; then
  echo "v$version is already released; nothing to do" >&2
  echo "release=false"
  exit 0
fi
bash scripts/release-notes.sh "$version" > "${NOTES:-/dev/null}"
echo "release=true"

#!/usr/bin/env bash
# Print the latest.json that installed copies of KeyHandoff check for updates:
# each package's download URL on the vVERSION release and its updater signature.
# Usage: scripts/updater-manifest.sh VERSION DIR NOTES_FILE  (DIR holds the
# packages and their .sig files)
set -euo pipefail
v="${1:?version}"; dir="${2:?dir}"; notes="${3:?notes file}"
base="https://github.com/junovhs/keyhandoff/releases/download/v$v"
entry() {
  local f="$dir/$1"
  [ -s "$f" ] && [ -s "$f.sig" ] || { echo "missing $1 or its signature" >&2; exit 1; }
  jq -n --arg url "$base/$1" --arg sig "$(cat "$f.sig")" '{url: $url, signature: $sig}'
}
win=$(entry "keyhandoff-$v-windows-x64-setup.exe")
appimage=$(entry "keyhandoff-$v-linux-x86_64.AppImage")
deb=$(entry "keyhandoff-$v-linux-amd64.deb")
jq -n --arg version "$v" --rawfile notes "$notes" --arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson win "$win" --argjson appimage "$appimage" --argjson deb "$deb" '{
    version: $version, notes: $notes, pub_date: $date,
    platforms: {
      "windows-x86_64-nsis": $win, "windows-x86_64": $win,
      "linux-x86_64-appimage": $appimage, "linux-x86_64": $appimage,
      "linux-x86_64-deb": $deb
    }
  }'

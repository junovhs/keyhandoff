#!/usr/bin/env bash
# Make double-clicking a .handoff file open a locally built KeyHandoff (Linux,
# current user only). The Windows installer registers the file type itself.
#
#   scripts/register-linux.sh [path/to/keyhandoff] [--profile NAME]
#   scripts/register-linux.sh --remove
set -euo pipefail

data="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop="$data/applications/handoff-dev.desktop"
mime="$data/mime/packages/handoff.xml"

if [[ "${1:-}" == "--remove" ]]; then
  rm -f "$desktop" "$mime"
  update-mime-database "$data/mime" >/dev/null 2>&1 || true
  update-desktop-database "$data/applications" >/dev/null 2>&1 || true
  echo "Removed the .handoff registration."
  exit 0
fi

here="$(cd "$(dirname "$0")/.." && pwd)"
bin="$(realpath "${1:-$here/src-tauri/target/release/keyhandoff}")"
shift || true
[[ -x "$bin" ]] || { echo "No Handoff binary at $bin; build it first (npx tauri build --no-bundle)." >&2; exit 1; }

mkdir -p "$(dirname "$desktop")" "$(dirname "$mime")"
cat > "$mime" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-handoff">
    <comment>Handoff key request or sealed key</comment>
    <glob pattern="*.handoff"/>
  </mime-type>
</mime-info>
XML

cat > "$desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=KeyHandoff (dev)
Exec="$bin" $* %f
MimeType=application/x-handoff;
NoDisplay=true
Terminal=false
DESKTOP

update-mime-database "$data/mime"
update-desktop-database "$data/applications" >/dev/null 2>&1 || true
xdg-mime default handoff-dev.desktop application/x-handoff
echo "Double-clicking a .handoff file now opens $bin $*"

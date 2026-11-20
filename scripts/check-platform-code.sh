#!/usr/bin/env bash
# ADR 0006: code that differs per OS lives only in src-tauri/src/os/. This
# fails when an OS condition (cfg(windows), cfg(unix), target_os, or cfg!()
# of them) appears anywhere else in src-tauri/src, outside a file's test
# module. Tests may pick fixtures per OS; behaviour may not.
#
# Usage: scripts/check-platform-code.sh [src-dir]   (default src-tauri/src)
set -euo pipefail

src="${1:-$(dirname "$0")/../src-tauri/src}"
found=0
while IFS= read -r -d '' file; do
  case "$file" in "$src"/os/*) continue ;; esac
  # Stop at the top-level test module: `#[cfg(test)]` then `mod tests`.
  hits=$(awk '
    prev ~ /^#\[cfg\(test\)\]/ && $0 ~ /^(pub(\(crate\))? )?mod tests/ { exit }
    # windows_subsystem in main.rs only marks the binary as a GUI program.
    $0 ~ /windows_subsystem/ { prev = $0; next }
    $0 ~ /cfg(_attr)?!?\(([^)]|\([^)]*\))*\<(windows|unix|target_os)\>/ { printf "%s:%d: %s\n", FILENAME, FNR, $0 }
    { prev = $0 }
  ' "$file")
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits"
    found=1
  fi
done < <(find "$src" -name '*.rs' -print0)

if [ "$found" -ne 0 ]; then
  echo
  echo "OS-specific code outside src-tauri/src/os/ (ADR 0006). Put it behind a function in"
  echo "src-tauri/src/os/mod.rs with one version in each of os/linux, os/windows and os/unsupported.rs."
  exit 1
fi
echo "Platform code is only in src-tauri/src/os/."

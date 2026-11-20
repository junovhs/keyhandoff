#!/usr/bin/env bash
# docs/parity.md (ADR 0006): every Windows, Linux or macOS cell is yes, n/a,
# or partly/no with an issue id, so no gap goes unowned. Where the Ishoo store
# is present (.ishoo/records/issues, kept out of git), each named issue must
# also still be open: a closed issue means the cell should now say yes.
#
# Usage: scripts/check-parity-doc.sh [repo-root]
set -euo pipefail

root="${1:-$(dirname "$0")/..}"
doc="$root/docs/parity.md"
records="$root/.ishoo/records/issues"
[ -d "$records" ] || records="$(git -C "$root" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)/../.ishoo/records/issues"
bad=0

# Table rows whose second column isn't the header or the divider.
while IFS= read -r line; do
  IFS='|' read -r _ feature win linux mac _ <<<"$line"
  for cell in "$win" "$linux" "$mac"; do
    cell="$(echo "$cell" | xargs)"
    case "$cell" in
      yes|n/a) ;;
      "no ("*")"|"partly ("*")")
        id="${cell#*(}"; id="${id%)}"
        if ! [[ "$id" =~ ^[A-Z]+-[0-9]+$ ]]; then
          echo "parity.md: '$cell' for$feature: name one issue id, like no (PAR-05)"; bad=1
        elif [ -d "$records" ]; then
          file="$records/$id.toml"
          if [ ! -f "$file" ]; then
            echo "parity.md: $id (for$feature) isn't an issue"; bad=1
          elif grep -Eq '^status = "(done|declined)"' "$file"; then
            echo "parity.md: $id (for$feature) is closed; update the cell"; bad=1
          fi
        fi ;;
      *) echo "parity.md: '$cell' for$feature: use yes, n/a, partly (ID) or no (ID)"; bad=1 ;;
    esac
  done
done < <(grep -E '^\|' "$doc" | grep -Ev '^\|[ -]*(Feature|Program)[ ]*\||^\|---')

if [ "$bad" -ne 0 ]; then exit 1; fi
if [ -d "$records" ]; then
  echo "parity.md: every gap names an open issue."
else
  echo "parity.md: every gap names an issue (no Ishoo store here to check they're open)."
fi

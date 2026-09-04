#!/usr/bin/env bash
# Usage: release-gate.sh <kind> <version>
# kind is "code" (v* tag) or "safety" (safety/v* tag).
# - SAFETY.md must exist with a "Version: <version>" header equal to the tag.
# - code releases additionally require a docs/safety/risk-report-* file.
set -euo pipefail

kind="${1:?usage: release-gate.sh <code|safety> <version>}"
version="${2:?usage: release-gate.sh <code|safety> <version>}"

[ -f SAFETY.md ] || {
  echo "release-gate: SAFETY.md not yet written (see ADR 0005); write it before tagging a release"
  exit 1
}
header="$(grep -m1 -E '^Version:[[:space:]]*[0-9]+\.[0-9]+\.[0-9]+' SAFETY.md | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true)"
[ "$header" = "$version" ] || {
  echo "release-gate: SAFETY.md Version '$header' != tag version '$version'"
  exit 1
}
echo "release-gate: SAFETY.md version $version OK"

if [ "$kind" = code ]; then
  if ! compgen -G "docs/safety/risk-report-*.md" > /dev/null; then
    echo "release-gate: no docs/safety/risk-report-*.md found (ADR 0006 requires one per release)"
    exit 1
  fi
  echo "release-gate: risk report present"
fi

echo "release-gate: PASS ($kind $version)"

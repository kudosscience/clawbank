#!/usr/bin/env bash
# Fail if docs/adr/ numbering is not a gapless sequence 0001..N,
# filenames do not match NNNN-slug.md, or a file lacks a Status section.
set -euo pipefail

dir="docs/adr"
[ -d "$dir" ] || { echo "adr-check: $dir missing"; exit 1; }

expected=1
fail=0
while IFS= read -r f; do
  base="$(basename "$f")"
  if [[ ! "$base" =~ ^([0-9]{4})-[a-z0-9-]+\.md$ ]]; then
    echo "adr-check: bad filename: $base (want NNNN-slug.md)"
    fail=1
    continue
  fi
  num="${BASH_REMATCH[1]}"
  want="$(printf '%04d' "$expected")"
  if [[ "$num" != "$want" ]]; then
    echo "adr-check: gap: found $base, expected ${want}-*.md"
    fail=1
  fi
  grep -q '^## Status' "$f" || { echo "adr-check: $base missing '## Status' section"; fail=1; }
  expected=$((expected + 1))
done < <(ls "$dir"/*.md | sort)

echo "adr-check: $((expected - 1)) ADRs, sequence OK"
exit "$fail"

#!/usr/bin/env bash
# Fail if CONTEXT.md is missing or has glossary lines that do not match
# "- **Term**: definition" (keeps the glossary machine-readable).
set -euo pipefail

f="CONTEXT.md"
[ -f "$f" ] || { echo "context-check: CONTEXT.md missing"; exit 1; }

fail=0
n=0
while IFS= read -r line; do
  [[ "$line" =~ ^-.* ]] || continue
  n=$((n + 1))
  if [[ ! "$line" =~ ^-\ \*\*[^*]+\*\*:\ .+ ]]; then
    echo "context-check: bad glossary line: $line"
    fail=1
  fi
done < "$f"

[ "$n" -gt 0 ] || { echo "context-check: no glossary entries found"; exit 1; }
echo "context-check: $n glossary entries, format OK"
exit "$fail"

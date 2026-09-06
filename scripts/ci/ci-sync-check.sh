#!/usr/bin/env bash
# Rule: CI rust jobs must invoke scripts/ci/rust-check.sh, never bare
# `cargo` commands, so the CI suite and the local suite stay in sync by
# construction instead of by convention.
#
# Usage: ci-sync-check.sh [workflow-file ...] (default: .github/workflows/ci.yml)
set -euo pipefail

if [ "$#" -gt 0 ]; then
  files=("$@")
else
  files=(.github/workflows/ci.yml)
fi
fail=0
for f in "${files[@]}"; do
  # 1. No bare `run: cargo ...` steps (the manifest-probe `cargo=` echos
  # and rust-check.sh invocations do not match on purpose).
  if grep -nE '^\s*(-\s+)?run:\s*cargo( |$)' "$f"; then
    echo "ci-sync-check: $f has bare cargo run-steps; use scripts/ci/rust-check.sh" >&2
    fail=1
  fi
  # 2. Every rust-check.sh <step> the workflow uses must be a step the
  # script implements.
  while IFS= read -r step; do
    if ! grep -qE "^\s*$step\)" scripts/ci/rust-check.sh; then
      echo "ci-sync-check: $f uses unknown rust-check.sh step '$step'" >&2
      fail=1
    fi
  done < <(grep -oE 'rust-check\.sh [a-z-]+' "$f" | awk '{print $2}' | sort -u)
done
# 3. Every shard step the script implements must be invoked by the scanned
# workflow(s) as a group, so a new check cannot land without CI running it
# and CI cannot silently drop one. (`all` is the local aggregator, not a
# CI shard.)
used="$(grep -hoE 'rust-check\.sh [a-z-]+' "${files[@]}" | awk '{print $2}' | sort -u)"
while IFS= read -r step; do
  if ! grep -qx "$step" <<<"$used"; then
    echo "ci-sync-check: no workflow invokes rust-check.sh step '$step'" >&2
    fail=1
  fi
done < <(bash scripts/ci/rust-check.sh --list-steps | tr ' ' '\n')
if [ "$fail" -eq 0 ]; then
  echo "ci-sync-check: OK"
fi
exit "$fail"

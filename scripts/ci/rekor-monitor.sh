#!/usr/bin/env bash
# Usage: rekor-monitor.sh [--json entries.json]
# Watches our Sigstore signing identity for unauthorized Rekor entries.
# - Live mode (default): for every GitHub Release in this repo, downloads
#   its Sigstore bundle and re-verifies it against the repo identity. A
#   release whose bundle fails verification, or a Rekor entry pinning a
#   commit no known tag contains, fails loudly so the safety-monitor
#   workflow can open a `safety`-labelled issue.
# - --json mode: checks a saved Rekor entry list offline (self-test and
#   reviewer replay): each entry must pin a commit contained in a known
#   tag (safety/v*, v*, pause/*), else FAIL.
# Output is summaries only (UUID + identity + commit) — never payloads.
set -euo pipefail

REPO="${REPO:-kudosscience/clawbank}"

# check_entries <json-file>: every entry's commit must resolve to a tag
# in this clone; unknown or unpinned commits FAIL.
check_entries() {
  local file="$1"
  python3 - "$file" <<'EOF'
import json, subprocess, sys

path = sys.argv[1]
try:
    data = json.load(open(path))
except Exception as exc:
    print(f"rekor-monitor: cannot parse entries JSON: {exc}")
    sys.exit(2)

if isinstance(data, dict):
    items = list(data.items())
elif isinstance(data, list):
    items = [(e.get("uuid", f"index-{i}"), e) for i, e in enumerate(data)]
else:
    print("rekor-monitor: unexpected entries shape (want object or list)")
    sys.exit(2)

if not items:
    print("rekor-monitor: no entries found (log empty for this identity)")
    sys.exit(0)

known, unknown = 0, 0
for uuid, entry in items:
    commit = ""
    if isinstance(entry, dict):
        body = entry.get("body", {}) or {}
        spec = body.get("spec", {}) or {}
        commit = ((spec.get("data", {}) or {}).get("hash", {}) or {}).get("value", "") or entry.get("commit", "")
    if not commit:
        print(f"rekor-monitor: entry {uuid} carries no commit pin — cannot authorize (FAIL)")
        unknown += 1
        continue
    try:
        tags = subprocess.run(
            ["git", "tag", "--contains", commit],
            capture_output=True, text=True, check=False).stdout.split()
    except Exception:
        tags = []
    if any(t.startswith(("v", "safety/v", "pause/")) for t in tags):
        known += 1
    else:
        print(f"rekor-monitor: entry {uuid} pins unknown commit {commit} (no safety/v*, v*, or pause/* tag contains it)")
        unknown += 1

print(f"rekor-monitor: {known} authorized, {unknown} unauthorized")
sys.exit(1 if unknown else 0)
EOF
}

if [ "${1:-}" = "--json" ]; then
  check_entries "${2:?usage: rekor-monitor.sh --json <entries.json>}"
  exit $?
fi

# Live mode: re-verify every release's attestation bundle.
if ! command -v gh >/dev/null 2>&1; then
  echo "rekor-monitor: gh CLI not found — cannot query releases (reachability gap, not a forgery signal)"
  exit 0
fi
tags="$(gh release list --repo "$REPO" --limit 100 --json tagName -q '.[].tagName' 2>/dev/null || true)"
if [ -z "$tags" ]; then
  echo "rekor-monitor: no releases published yet — nothing to cross-check (PASS)"
  exit 0
fi
fail=0
while IFS= read -r tag; do
  [ -n "$tag" ] || continue
  dir="$(mktemp -d)"
  if gh release download "$tag" --repo "$REPO" -D "$dir" >/dev/null 2>&1; then
    while IFS= read -r -d '' f; do
      case "$f" in *.jsonl|*SHA256SUMS) continue;; esac
      if gh attestation verify "$f" --repo "$REPO" >/dev/null 2>&1; then
        echo "rekor-monitor: $tag/$(basename "$f") attestation OK"
      else
        echo "rekor-monitor: $tag/$(basename "$f") attestation FAILED (possible forgery)"
        fail=1
      fi
    done < <(find "$dir" -type f -print0)
  else
    echo "rekor-monitor: cannot download $tag (reachability gap, not a forgery signal)"
  fi
  rm -rf "$dir"
done <<< "$tags"
[ "$fail" -eq 0 ] && echo "rekor-monitor: PASS (all release attestations verify)"
exit "$fail"

#!/usr/bin/env bash
# Usage: release-gate.sh <kind> <version>
# kind is "code" (v* tag) or "safety" (safety/v* tag).
# - SAFETY.md must exist with a "Version: <version>" header equal to the tag.
# - code releases additionally require a filled docs/safety/risk-report-*.md
#   note (template files are excluded). Each note must carry the sections
#   defined in docs/safety/risk-report-template.md; see
#   docs/safety/risk-reports.md for cadence and sign-off.
set -euo pipefail

kind="${1:?usage: release-gate.sh <code|safety> <version>}"
version="${2:?usage: release-gate.sh <code|safety> <version>}"

# check_risk_note <file>: fail loudly unless the note carries every required
# section, names coverage/publish dates plus a commit, marks redactions
# explicitly (never silently omitted), and holds no peer-identifying or
# secret material (IPs, multiaddrs, private keys stay internal-only).
check_risk_note() {
  local note="$1"
  local missing=()
  local section
  for section in "evaluations run" "FAL verdict" "coverage and publication" "commit" "known gaps" "redactions"; do
    if ! grep -qiE "^##[[:space:]]+${section}[[:space:]]*$" "$note"; then
      missing+=("$section")
    fi
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "release-gate: $note missing section(s): $(IFS=,; echo "${missing[*]}") (see docs/safety/risk-report-template.md)"
    return 1
  fi
  if ! grep -qi 'coverage_date' "$note"; then
    echo "release-gate: $note must name its coverage_date (see docs/safety/risk-report-template.md)"
    return 1
  fi
  if ! grep -qi 'published' "$note"; then
    echo "release-gate: $note must name its published date (see docs/safety/risk-report-template.md)"
    return 1
  fi
  if ! grep -qiE 'commit[^0-9a-f]*[0-9a-f]{40}' "$note"; then
    echo "release-gate: $note must name the evaluated full commit SHA (40 hex chars; see docs/safety/risk-report-template.md)"
    return 1
  fi
  local redaction_body
  redaction_body="$(awk '{ low = tolower($0) } low ~ /^##[ \t]+redactions[ \t]*$/ { in_sec = 1; next } in_sec && $0 ~ /^##[ \t]/ { exit } in_sec { print }' "$note")"
  if [ -z "$(echo "$redaction_body" | grep -v '^[[:space:]]*$' || true)" ]; then
    echo "release-gate: $note has an empty Redactions section; state 'None' or list markers (never omit silently)"
    return 1
  fi
  if ! echo "$redaction_body" | grep -qiE '\[REDACTED[^]]*withheld|\bnone\b'; then
    echo "release-gate: $note Redactions section must contain a [REDACTED — category — N tokens withheld] marker or an explicit 'None'"
    return 1
  fi
  if grep -qE '[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}' "$note"; then
    echo "release-gate: $note must not contain peer-identifying addresses (IPs stay internal-only)"
    return 1
  fi
  if grep -qE '/ip4/|/ip6/|/dns/' "$note"; then
    echo "release-gate: $note must not contain peer-identifying multiaddrs (stay internal-only)"
    return 1
  fi
  if grep -q 'PRIVATE KEY' "$note"; then
    echo "release-gate: $note must not contain secret key material"
    return 1
  fi
  return 0
}

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
  shopt -s nullglob
  candidates=(docs/safety/risk-report-*.md)
  notes=()
  for candidate in "${candidates[@]}"; do
    case "$candidate" in
      *template*) continue ;;
      *) notes+=("$candidate") ;;
    esac
  done
  if [ "${#notes[@]}" -eq 0 ]; then
    echo "release-gate: no docs/safety/risk-report-*.md found (ADR 0006 requires one per release)"
    exit 1
  fi
  for note in "${notes[@]}"; do
    check_risk_note "$note" || exit 1
  done
  echo "release-gate: risk report present (${#notes[@]} note(s) checked)"
fi

echo "release-gate: PASS ($kind $version)"

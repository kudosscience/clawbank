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

# note_iso_date_ok <file> <field>: the field names a real YYYY-MM-DD
# calendar date (placeholders such as TBD fail; impossible dates fail).
note_iso_date_ok() {
  local file="$1"
  local field="$2"
  local line day
  line="$(grep -m1 -i -E "${field}[^0-9]*[0-9]{4}-[0-9]{2}-[0-9]{2}" "$file" || true)"
  [ -n "$line" ] || return 1
  day="$(echo "$line" | grep -m1 -o -E '[0-9]{4}-[0-9]{2}-[0-9]{2}' | head -n1 || true)"
  [ -n "$day" ] || return 1
  [ "$(date -d "$day" +%F 2>/dev/null || true)" = "$day" ] || return 1
  return 0
}

# check_risk_note <file>: fail loudly unless the note carries every required
# section, names real coverage/publish dates plus an isolated full commit
# SHA, ties gaps to tickets, marks redactions explicitly (never silently
# omitted), and holds no peer-identifying or secret material (IP literals,
# multiaddrs, private keys stay internal-only).
check_risk_note() {
  local note="$1"
  local missing=()
  local section
  for section in "evaluations run" "FAL verdict" "coverage and publication" "commit" "known gaps" "redactions" "attestation"; do
    if ! grep -qiE "^##[[:space:]]+${section}[[:space:]]*$" "$note"; then
      missing+=("$section")
    fi
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "release-gate: $note missing section(s): $(IFS=,; echo "${missing[*]}") (see docs/safety/risk-report-template.md)"
    return 1
  fi
  local field
  for field in coverage_date published; do
    if ! note_iso_date_ok "$note" "$field"; then
      echo "release-gate: $note must give $field as a real YYYY-MM-DD calendar date (see docs/safety/risk-report-template.md)"
      return 1
    fi
  done
  if ! grep -qiE 'commit[^0-9a-f]*([^0-9a-f]|^)[0-9a-f]{40}([^0-9a-f]|$)' "$note"; then
    echo "release-gate: $note must name the evaluated full commit SHA as an isolated 40-hex-char string (see docs/safety/risk-report-template.md)"
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
  if grep -qE '(^|[^[:alnum:]:])(([0-9A-Fa-f]{1,4}:){7}[0-9A-Fa-f]{1,4}|([0-9A-Fa-f]{1,4}:){1,7}:|([0-9A-Fa-f]{1,4}:){1,6}:[0-9A-Fa-f]{1,4}|([0-9A-Fa-f]{1,4}:){1,5}(:[0-9A-Fa-f]{1,4}){1,2}|([0-9A-Fa-f]{1,4}:){1,4}(:[0-9A-Fa-f]{1,4}){1,3}|([0-9A-Fa-f]{1,4}:){1,3}(:[0-9A-Fa-f]{1,4}){1,4}|([0-9A-Fa-f]{1,4}:){1,2}(:[0-9A-Fa-f]{1,4}){1,5}|[0-9A-Fa-f]{1,4}:((:[0-9A-Fa-f]{1,4}){1,6})|:((:[0-9A-Fa-f]{1,4}){1,7}|:))([^[:alnum:]:]|$)' "$note"; then
    echo "release-gate: $note must not contain peer-identifying addresses (IPs stay internal-only)"
    return 1
  fi
  if grep -qE '/ip4/|/ip6/|/dns/' "$note"; then
    echo "release-gate: $note must not contain peer-identifying multiaddrs (stay internal-only)"
    return 1
  fi
  # Every Known-gaps item (a "- " bullet plus its continuation lines)
  # must name a tracking ticket, so gaps stay traceable.
  if ! awk '
    function flush() { if (have && item !~ /#[0-9]+/) bad = 1; item = ""; have = 0 }
    { low = tolower($0) }
    !started && low ~ /^##[ \t]+known gaps[ \t]*$/ { started = 1; next }
    started && !done && $0 ~ /^##[ \t]/ { flush(); done = 1; next }
    !started || done { next }
    $0 ~ /^[ \t]*-[ \t]+/ { flush(); have = 1; seen = 1; item = $0; next }
    have { item = item "\n" $0 }
    END { flush(); if (!started || !seen || bad) exit 1 }
  ' "$note"; then
    echo "release-gate: $note Known gaps items must each name a tracking ticket (e.g. #41; see docs/safety/risk-report-template.md)"
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

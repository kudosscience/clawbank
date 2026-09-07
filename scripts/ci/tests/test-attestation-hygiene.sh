#!/usr/bin/env bash
# Self-test for scripts/ci/attestation-hygiene.sh (TDD seam for Docs 03).
# Run: bash scripts/ci/tests/test-attestation-hygiene.sh
set -euo pipefail

script="scripts/ci/attestation-hygiene.sh"
pass=0
fail=0

expect_pass() {
  local name="$1" dir="$2"
  if bash "$script" "$dir" >/dev/null 2>&1; then
    pass=$((pass + 1))
  else
    echo "FAIL (expected PASS): $name"
    fail=$((fail + 1))
  fi
}

expect_fail() {
  local name="$1" dir="$2"
  if bash "$script" "$dir" >/dev/null 2>&1; then
    echo "FAIL (expected FAIL): $name"
    fail=$((fail + 1))
  else
    pass=$((pass + 1))
  fi
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Clean bundle: digests plus summaries only.
mkdir -p "$tmp/clean"
printf 'FAL-2 reaffirmed.\nsha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\nNone — no material was withheld.\n' > "$tmp/clean/note.md"
expect_pass "clean summaries pass" "$tmp/clean"

# Real risk note + runbook must pass (guards against self-trip).
mkdir -p "$tmp/repo"
cp docs/safety/risk-report-2026-09-07.md "$tmp/repo/" 2>/dev/null || true
cp docs/safety/verify-runbook.md "$tmp/repo/" 2>/dev/null || true
cp docs/safety/MAINTAINER_PUBKEYS.asc "$tmp/repo/" 2>/dev/null || true
expect_pass "in-repo safety docs pass" "$tmp/repo"

# Forbidden fixtures: each must fail.
mkdir -p "$tmp/v4" && printf 'peer at 192.168.1.10 responded\n' > "$tmp/v4/note.md"
expect_fail "ipv4 literal fails" "$tmp/v4"

mkdir -p "$tmp/v6" && printf 'peer at 2001:db8::1 responded\n' > "$tmp/v6/note.md"
expect_fail "ipv6 literal fails" "$tmp/v6"

mkdir -p "$tmp/ma" && printf 'relay /ip4/10.0.0.1/tcp/4001 here\n' > "$tmp/ma/note.md"
expect_fail "multiaddr fails" "$tmp/ma"

mkdir -p "$tmp/key" && printf -- '-----BEGIN PRIVATE KEY-----\nabc\n' > "$tmp/key/note.md"
expect_fail "private key fails" "$tmp/key"

# Loopback/unspecified bind literals are not peer-identifying.
mkdir -p "$tmp/lo" && printf 'bind 127.0.0.1:0, never 0.0.0.0; loopback ::1 ok\n' > "$tmp/lo/note.md"
expect_pass "loopback docs pass" "$tmp/lo"

# ... but real peer addresses sharing a suffix still fail.
mkdir -p "$tmp/edge" && printf 'peer at 2001:db8::1 responded\n' > "$tmp/edge/v6.md"
printf 'relayed 192.168.127.1 last night\n' > "$tmp/edge/v4.md"
expect_fail "peer addrs with loopback-like suffixes fail" "$tmp/edge"

# Design-doc idioms that name no peer: OIDs, version tuples, placeholder
# multiaddrs (mirrors docs/adr/0002 + 0008, which must keep passing).
mkdir -p "$tmp/idiom"
printf 'OID 1.3.6.1.4.1.53594.1.1, libp2p 0.54.1, bootstrap /ip4/.../p2p/<PeerId> or /dnsaddr\n' > "$tmp/idiom/adr.md"
expect_pass "oid/version/placeholder idioms pass" "$tmp/idiom"

# ... while a multiaddr with a real address still fails.
mkdir -p "$tmp/realma" && printf 'relay /ip4/10.0.0.1/tcp/4001 here\n' > "$tmp/realma/note.md"
expect_fail "real multiaddr fails" "$tmp/realma"

echo "attestation-hygiene self-test: $pass passed, $fail failed"
[ "$fail" -eq 0 ]

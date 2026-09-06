#!/usr/bin/env bash
# Canonical Rust quality gate. CI invokes the fmt|clippy|test|audit shards
# (never bare `cargo` commands) so the CI suite and the local suite cannot
# drift; `all` is the local aggregator chaining the same shards.
# scripts/ci/ci-sync-check.sh enforces both directions of that rule.
#
# Usage: rust-check.sh [all|fmt|clippy|test|audit|--list-steps]
set -euo pipefail

step="${1:-all}"
case "$step" in
  all|fmt|clippy|test|audit|--list-steps) ;;
  -h|--help|help) sed -n '2,8p' "$0"; exit 0 ;;
  *)
    echo "rust-check: unknown step '$step' (want all|fmt|clippy|test|audit)" >&2
    exit 2
    ;;
esac

if [ "$step" = "--list-steps" ]; then
  echo "fmt clippy test audit"
  exit 0
fi

[ -f Cargo.toml ] || { echo "rust-check: no Cargo.toml in $PWD" >&2; exit 1; }

# Enforce lockfile currency when a lockfile exists; fresh workspaces
# without one yet still build (CI gates audit on Cargo.lock separately).
lock_args=()
[ -f Cargo.lock ] && lock_args=(--locked)

run_fmt() { cargo fmt --all -- --check; }
run_clippy() { cargo clippy "${lock_args[@]}" --all-targets -- -D warnings; }
run_test() { cargo test "${lock_args[@]}" --all; }
run_audit() {
  [ -f Cargo.lock ] || { echo "rust-check: no Cargo.lock" >&2; exit 1; }
  command -v cargo-audit >/dev/null || {
    echo "rust-check: cargo-audit not installed (cargo install cargo-audit --locked)" >&2
    exit 1
  }
  command -v cargo-deny >/dev/null || {
    echo "rust-check: cargo-deny not installed (cargo install cargo-deny --locked)" >&2
    exit 1
  }
  cargo audit
  cargo deny check advisories bans licenses sources
}

case "$step" in
  all) run_fmt && run_clippy && run_test && run_audit ;;
  fmt) run_fmt ;;
  clippy) run_clippy ;;
  test) run_test ;;
  audit) run_audit ;;
esac
echo "rust-check: $step OK"

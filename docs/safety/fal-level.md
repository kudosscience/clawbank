# FAL level and the pause rule

The network ships at **FAL-2** (virtual credit network, ADR-0004). The level
is declared mechanically in one constant —
`clawbank_safety::FAL_LEVEL` in `crates/clawbank-safety/src/lib.rs` (currently
`2`) — and enforced by the gate test in
`crates/clawbank-safety/tests/fal_gate.rs`.

## What the gate does

While the level is below 3, the gate fails the build if any of these
FAL-3-gated capabilities appears anywhere in the tree (every file at any
depth, including `Cargo.toml` manifests — only `.git/`, `target/`, hidden
tooling directories, the generated `Cargo.lock`, and the allowlisted paths
below are skipped):

- escrow (including HTLC and multi-sig escrow)
- lending and borrowing (loans; bare `borrow`/`lend` are excluded — every
  `borrow_mut` and `blend` would trip them — while `borrowing`, `borrower`,
  `lender`, and kin carry the signal)
- margin and yield
- fiat and crypto bridges
- compute marketplace
- policy-based autonomous spending

Matching is case-, separator-, and camelCase-insensitive
(`ComputeMarketplace`, `compute_marketplace`, and `computemarketplace`
all match), so identifiers cannot dodge the gate by picking separators.

Only the gate-owned files (`crates/clawbank-safety/src/lib.rs` and
`crates/clawbank-safety/tests/fal_gate.rs`) may name these terms in code:
they own the constant and the gate's term list. `SAFETY.md` (repo-root
commitment), `docs/adr/`, and `docs/safety/` may discuss gated capabilities
freely — prose proposes, code implements, and only unallowlisted matches
trip the gate.

The gate runs in the existing CI required checks: the `rust-test` job invokes
`scripts/ci/rust-check.sh test` (`cargo test --all`), so every PR runs it and
no workflow change is needed to keep it enforced.

## Demoed both directions

- Red: adding `crates/clawbank/src/escrow_stub.rs` (an `Escrow` struct) makes
  `no_fal3_capabilities_below_fal3` fail, naming the file and the term.
- Green: removing the stub makes the full gate suite pass again.

## Raising the level (pause rule)

Flipping `FAL_LEVEL` without evidence changes nothing — the pin test holds the
value and this procedure still applies. A rise to FAL-3 requires, in order:

1. **Safeguards** — the ADR-0004 FAL-3 defense-in-depth bundle (spending
   caps/allow-lists, inline anomaly classifier, async ledger analysis,
   pause/freeze rapid response) plus the 17-control subset (multi-party
   mint/bridge auth, SLSA provenance, audit log/SIEM, honeypot escrow,
   external red-team).
2. **Evaluations** — the ADR-0006 FAL-3 harness passes, showing no meaningful
   catastrophic misuse under adversarial testing.
3. **Reviewer** — a maintainer plus an external reviewer sign off, recorded in
   `SAFETY.md` (Appendix C + Changelog) and a per-release Risk Report.

Only then may `FAL_LEVEL` be raised and the gate's allowlist updated.

//! Financial Autonomy Level (FAL) gate — ADR-0004 enforcement.
//!
//! The network ships at FAL-2 (virtual credit network). [`FAL_LEVEL`] is the
//! single mechanical declaration of that level; the pause-rule gate test in
//! `tests/fal_gate.rs` fails the build the moment a FAL-3-gated capability
//! appears anywhere outside the allowlisted safety docs while the level is
//! still below 3.
//!
//! ## Raising the level (pause rule, not a constant flip)
//!
//! Setting [`FAL_LEVEL`] to 3 without evidence changes nothing — the gate
//! test pins the value and the procedure below still applies. A level rise
//! requires, in order:
//!
//! 1. Safeguards: the FAL-3 defense-in-depth bundle from ADR-0004
//!    (spending caps/allow-lists, inline anomaly classifier, async ledger
//!    analysis, pause/freeze rapid response) plus the 17-control subset
//!    (multi-party mint/bridge auth, SLSA provenance, audit log/SIEM,
//!    honeypot escrow, external red-team).
//! 2. Evaluations: the FAL-3 harness from ADR-0006 passes, demonstrating no
//!    meaningful catastrophic misuse under adversarial testing.
//! 3. Reviewer: a maintainer plus an external reviewer sign off, recorded in
//!    `SAFETY.md` (Appendix E + Changelog) and a per-release Risk Report.
//!
//! Only then may [`FAL_LEVEL`] be raised and the gate's allowlist updated.
//! Full operator procedure: `docs/safety/fal-level.md`.

/// Financial Autonomy Level the codebase is currently allowed to implement.
///
/// `2` = virtual credit network (MVP): fixed-supply virtual credits, P2P
/// transfers, reputation. Any escrow, lending, margin/yield, bridge,
/// compute-marketplace, or policy-based autonomous spending requires FAL-3
/// and its safeguards first — see the module docs and
/// `docs/safety/fal-level.md`.
pub const FAL_LEVEL: u8 = 2;

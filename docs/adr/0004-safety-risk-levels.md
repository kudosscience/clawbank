# ADR 0004: Safety risk levels are Financial Autonomy Levels (FAL-1..4), MVP ships at FAL-2

AI Bank adopts Financial Autonomy Levels (FAL) modelled on Anthropic's ASL: FAL-1 Sandbox → FAL-2 Virtual credit network (MVP, fixed-supply virtual credits + reputation, no leverage) → FAL-3 Real-value-adjacent (escrow, lending, fiat/crypto bridges) → FAL-4 Autonomous macro-economy (undefined). MVP is explicitly bounded to FAL-2 with contained blast radius and proportionate safeguards; crossing to FAL-3 requires maintainer-reviewed pause gate — see `research/safety-risk-assessment` decision record.

## Status

Accepted — implements wayfinder ticket [#5 Safety risk assessment: harm scenarios for agent financial autonomy](https://github.com/kudosscience/clawbank/issues/5) → `research/safety-risk-assessment` (`docs/research/safety-risk-assessment.md:1` on `research/safety-risk-assessment`).

## Context

LLM agents with financial autonomy amplify scale + speed + coordination: 12 concrete harm families have deployed precedents (Sybil, collusion, reputation gaming, funding harmful tasks, dust/DoS, forks/double-spend, inflation, cornering, griefing, linkability, laundering, runaway loops). MVP must be shippable without real-world financial loss, while future features (escrow, lending, bridges, autonomous earn loops) unlock qualitatively higher risk. A levels system analogous to RSP's ASL provides reversible triggers, safeguard bundles, and a pause rule without over-engineering the MVP.

## Considered Options

- **4-level FAL with pause rule (chosen)** — Each level = capability threshold + required safeguards (deployment + security + governance), monotonic inheritance, re-evaluated on safeguard changes. Borrowed from RSP v2.0 (safeguards not model labels) and BSL-inspired pause creating "race to top" incentive. Blast radius analysis proves FAL-2 worst case is recoverable ledger corruption / reputation collapse / DoS via social fork, not catastrophic.
- **No levels / ad-hoc reviews (rejected)** — allows FAL-3 features to slip in without gate; no public justification analogous to RSP.
- **Copy ASL verbatim (rejected)** — ASL targets CBRN/autonomy misuse of foundation models; AI Bank needs financial-autonomy thresholds (bridges, leverage, recursive earn loops).

## FAL definitions (normative)

- **FAL-1 Sandbox** — Single-node/LAN, non-transferable or trivial balances, no P2P/reputation. No safeguards beyond local `0o600` key storage.
- **FAL-2 Virtual credit network (MVP) ← SHIP HERE** — Fixed-supply virtual credits, genesis mint only, P2P transfers over libp2p, replicated ledger, reputation from history. Harms #1–#6 + #9 bounded. Required safeguards: typed validated transfers (dust threshold, nonce, Ed25519 sig, supply invariant), per-peer rate limits + gossipsub scoring, tenure-weighted decay + diversity + whitewash cost (new PeerId = zero reputation), explicit fork-choice + social-fork checkpoint, `libp2p_identity` key gen + Noise/TLS PeerId verify, relay `Limit{duration,data}`, `cargo audit`/`deny`. No multi-party ledger auth / SIEM / honeypots needed at this level. Evaluation: `cargo test` transaction integrity, Sybil N=50, circular-trade/whitewash, dust flood — all local. Trigger to FAL-3: any escrow, lending, margin/yield, bridge, or policy-based autonomous spending.
- **FAL-3 Real-value-adjacent** — Escrow/HTLC/multi-sig, lending/borrowing, fiat/crypto bridges, compute marketplace. Unlocks real financial loss, laundering, cornering. Requires defense-in-depth (spending caps/allow-lists → inline anomaly classifier → async ledger analysis → pause/freeze rapid response) + 17-control subset (multi-party mint/bridge auth, SLSA provenance, audit log/SIEM, honeypot escrow, external red-team), Risk Reports + external reviewer, 6-month cadence. Must show no meaningful catastrophic misuse under adversarial testing before shipping.
- **FAL-4 Autonomous macro-economy** — Recursive credit→compute earn loops persisting without human re-auth, systemic external-market impact. Undefined today; commit to define before reaching FAL-3 (RSP ASL-4 analogue) — expected: interpretability-style goal assurance, hard autonomy caps, survivable kill-switch.

Full harm catalogue, precedents (Douceur 2002, MDPI 16(14):6929, Halborn 2026, Bitcoin double-spend taxonomy), blast-radius table, and RSP→FAL mapping are in the research doc; this ADR locks the level structure and placement.

## Consequences

- Public safety commitment: maintain `SAFETY.md` / Risk Reports modelled on RSP changelog and public redacted reports (informs #6).
- Evaluation framework (#7) must implement FAL-2 runnable harness; FAL-3+ evaluations block feature landing via pause rule.
- Ledger/reputation code must implement FAL-2 safeguards as requirements, not optional features (dust filter, supply invariant, fork-choice, decay).
- Governance: maintainer decides FAL level per #1, publishes FAL definition, with non-retaliation for raising concerns; threshold re-evaluated on every safeguard upgrade.
- Future `docs/research/safety-risk-assessment.md` promotion to repo-root docs if needed for auditable link from `SAFETY.md`.

# ADR 0006: FAL safety evaluations are local cargo-test harness per-release + 6-month heartbeat, with PR-gated escalation

FAL-2 safety is proved by a local `cargo test` harness (transaction integrity, Sybil N=50, reputation circular/whitewash, dust flood vs relay caps, plus supply-invariant/fork-choice liveness), run every FAL-relevant release and at least every 6 months, with pause-trigger and coverage dates. Escalation FAL-2→3 is any escrow/lending/bridge/autonomous-spending proposal; FAL-3 requires four-layer deployment + security subset + Risk Report + external reviewer `no meaningful catastrophic misuse` before merge; emergency pause via signed `pause/YYYY-MM-DD` tag — see evaluation decision record.

## Status

Accepted — implements wayfinder ticket [#7 Safety evaluation framework: what proves safety at each risk level](https://github.com/kudosscience/ai-bank/issues/7) (grilling, dependency ADR 0004 FAL-1..4). Depends on ADR 0004 (FAL) and ADR 0005 (SAFETY.md/Risk Reports/git+Rekor).

## Context

Maintainer governs safety (per Destination) under a lightweight RSP-like regime (ADR 0005). Evaluations must be depth-over-frequency, locally runnable with no cloud bills (nodes on user hardware, libp2p + localhost HTTP), and legible to an external reviewer without trusting a hosted evaluator. Prior ADRs fix FAL-2 safeguards (dust threshold, nonce/replay, supply invariant, fork-choice + social-fork, tenure-weighted decay, peer-score, relay caps) but not the harness that proves them.

## Considered Options

- **Local cargo harness per-release + heartbeat + gated escalation (chosen)** — `cargo test --test safety_fal2` exercises: (a) transaction integrity (double-spend rejected, replay/nonce rejected, sum-to-supply invariant under partition, fork-choice deterministic + checkpoint), (b) Sybil spot-check N=50 measuring reputation/distribution distortion, (c) reputation gaming (circular trade + whitewash new PeerId→zero reputation + diversity flag), (d) resource exhaustion (dust flood 10× rate, relay `Limit{duration,data}` + gossipsub scoring holds, `cargo audit`/`deny` clean). Paired with pause-trigger test (propose escrow/lending/bridge → harness flags `FAL-3 gate required`). Cadence: every release touching `crates/*` or transfer/reputation logic + 6-month `docs/safety/updates.md` heartbeat even with no release + within 7 days on pause (depth over frequency, RSP v2.0 3→6 mo lesson). Artefacts: `docs/safety/risk-report-YYYY-MM-DD.md` (redacted, `coverage_date` vs `published`, `[REDACTED]` markers, `commit` + `FAL: 2`) linked from `CHANGELOG.md`/Release, attested via Sigstore/Rekor SLSA bundle (digest + source URI + builder ID + commit). Review: standard `SAFETY.md` edit → maintainer PR; FAL threshold change → maintainer + external reviewer + Appendix C + Changelog MINOR; FAL-3 feature → blocked until safeguards + harness pass + reviewer; emergency `git tag -s pause/YYYY-MM-DD` without full review → `safety`-labelled issue + 7-day full report, anti-retaliation. References ADR 0005 auditability (`git verify-tag`, `slsa-verifier`/`cosign`/`rekor-cli`) and RSP §3.5/Seoul VII redaction rules.
- **Continuous/hosted evaluator or hosted SIEM dashboard (rejected)** — requires central infra and bill, violates "no cloud bills / forkable" walkaway test; overkill for FAL-2 contained blast radius (ADR 0004).
- **Ad-hoc on-demand only with no heartbeat (rejected)** — no coverage guarantee; RSP v3 cadence shows heartbeat prevents silent drift.

## Consequences

- `SAFETY.md` §§3–6 import this harness by reference (not inline); `SAFETY.md` FAL-2 justification cites `research/safety-risk-assessment` blast-radius table + this ADR's harness names.
- Any `escrow`/`lending`/`fiat bridge`/`autonomous spending policy` PR is gated by pause rule (ADR 0004/0005 §6.2) until FAL-3 four-layer + security subset + external red-team `no meaningful catastrophic misuse` demonstrated.
- Evaluations are runnable by any operator (`cargo test`) and verifiable offline from bundle + TUF root (ADR 0005 §6); raw exploit payloads/peer IPs never enter Rekor.
- Annual procedural self-review (`docs/safety/procedural-review-YYYY.md`) checks did-we-follow-§§3–7; third-party review only at FAL-3+ (ADR 0005).
- FAL-4 remains undefined by design (commit to define before reaching FAL-3).

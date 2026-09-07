# ClawBank Safety Policy (SAFETY.md)

Version: 0.1.0
Effective: 2026-09-07

> Version: 0.1.0 | Effective: 2026-09-07 | Supersedes: (none) | Branch: `main`
> | Maintainer/RSO: [@kudosscience](https://github.com/kudosscience) (Henry Ward)
> | Feedback: GitHub Issues with the `safety` label
> | Source of truth for levels: [ADR-0004](docs/adr/0004-safety-risk-levels.md)
> | Argument track: [MVP safety case](docs/safety/mvp-safety-case.md) (Safety 02, #36)
> | Document pattern: [ADR-0005](docs/adr/0005-safety-doc-pattern.md) | Evaluations: [ADR-0006](docs/adr/0006-safety-evaluation-framework.md)

## Preamble

ClawBank is a decentralised credit network for LLM agents with no board,
no cloud bills, and no central authority. This file is the durable public
safety commitment: what the network may do, what safeguards that requires,
why the MVP is safe, and how the commitment itself is versioned and
audited.

It follows the Responsible Scaling Policy grammar (public commitment, not
platform policy): proportional, iterative, and exportable. Like a BIP, it
serves as a collaborative record for proposing safety standards which node
operators voluntarily adopt by running satisfying code — forks remain
possible, and safety is achieved by legibility of the fork, not by central
enforcement.

Safety is part of the MVP with maintainer (RSO) governance. Commitments
here are forkable, locally verifiable, and survive without a hosted
service. Public by default, redact by rule: thresholds, safeguards, and
evaluation summaries are public; raw exploit detail and peer-identifying
data stay internal and redactions are marked, never silent
(per [ADR-0005](docs/adr/0005-safety-doc-pattern.md) and
[risk-reports.md](docs/safety/risk-reports.md)).

This is Docs 01 core ([#38](https://github.com/kudosscience/clawbank/issues/38)):
preamble, FAL background, the threshold/safeguard matrix, the Safety 02
justification core, versioning, and the Changelog. Operating assessment
procedures, full deployment/governance text, and Appendices A–C/E–F land in
Docs 02 ([#39](https://github.com/kudosscience/clawbank/issues/39));
signing, tags, and attestation land in Docs 03
([#40](https://github.com/kudosscience/clawbank/issues/40)). Those tracks
extend this file — they do not fork its content.

## 1. Background — the FAL model (imported, not restated)

Normative level definitions live in exactly one place:
[ADR-0004](docs/adr/0004-safety-risk-levels.md). This section imports them
by reference and never restates them. Any wording below that looks like a
definition is a non-normative summary; on any divergence ADR-0004 wins.

Financial Autonomy Levels (FAL) are sets of required safeguards tied to
reversible capability thresholds, modelled on Anthropic's ASL: FAL-1
Sandbox, FAL-2 Virtual credit network (MVP), FAL-3 Real-value-adjacent,
FAL-4 Autonomous macro-economy (undefined today; committed to define before
reaching FAL-3). Each level inherits the previous bundle monotonically and
is re-evaluated whenever safeguards upgrade.

The network ships at **FAL-2**. The level is declared mechanically in one
constant — `clawbank_safety::FAL_LEVEL` in
`crates/clawbank-safety/src/lib.rs` (currently `2`) — pinned by test and
enforced by the pause-rule gate in
`crates/clawbank-safety/tests/fal_gate.rs`. Operator procedure:
[fal-level.md](docs/safety/fal-level.md).

The full harm catalogue (12 families with deployed precedents), the
blast-radius table, and the RSP-to-FAL mapping are in the ADR-0004 research
record; this file locks nothing beyond pointing at them.

## 2. Capability thresholds and required safeguards

Source of truth: [ADR-0004](docs/adr/0004-safety-risk-levels.md). The table
below is a non-normative summary of it — it creates no second normative
copy. The "Required safeguards" column states what each level *requires*,
not what is implemented today (implementation status lives in §4 and
[§5b of the safety case](docs/safety/mvp-safety-case.md#5b-planned--explicitly-not-done-tracked-by-ticket)).

| Level | Name | What agents may do | Required safeguards (summary) |
| ----- | ---- | ------------------ | ----------------------------- |
| FAL-1 | Sandbox | Single-node/LAN, non-transferable or trivial balances, no P2P/reputation | Local `0o600` key storage only |
| FAL-2 | Virtual credit network (**MVP — ship here**) | Fixed-supply virtual credits, genesis mint only, P2P transfers, replicated ledger, history-derived reputation | Typed validated transfers, rate limits + message scoring, tenure-weighted reputation with whitewash cost, explicit fork-choice + social-fork checkpoint, authenticated transport, relay caps, `cargo audit`/`deny` |
| FAL-3 | Real-value-adjacent | Escrow (incl. HTLC/multi-sig), lending/borrowing, margin/yield, fiat/crypto bridges, compute marketplace, policy-based autonomous spending | FAL-2 bundle **plus** defense-in-depth (spending caps/allow-lists, inline anomaly classifier, async ledger analysis, pause/freeze rapid response) **plus** the 17-control subset (multi-party mint/bridge auth, SLSA provenance, audit log/SIEM, honeypot escrow, external red-team), Risk Reports + external reviewer |
| FAL-4 | Autonomous macro-economy | Recursive credit-to-compute earn loops persisting without human re-auth, systemic external-market impact | Undefined today; committed to define before reaching FAL-3 |

Re-evaluation commitment: thresholds are re-visited whenever safeguards
upgrade. Crossing to FAL-3 requires the pause-rule evidence in order —
safeguards (ADR-0004 FAL-3 bundle + 17-control subset), evaluations
(ADR-0006 FAL-3 harness showing no meaningful catastrophic misuse),
reviewer (maintainer plus external, recorded here and in a per-release Risk
Report) — never a bare constant flip.

## 3. Justification core — why FAL-2 is safe (housed from Safety 02)

This section houses the published Safety 02 argument
([mvp-safety-case.md](docs/safety/mvp-safety-case.md), [#36](https://github.com/kudosscience/clawbank/issues/36))
as the justification core of this commitment. It imports that file's
reasoning without duplicating it: summaries below point at the safety case
for the full text, and safeguard statuses point at its §5 legend
(**[Implemented]** / **[Planned — #N]** / **[Procedural]**).

### 3.1 Placement: the MVP is FAL-2

The MVP's design scope is a virtual credit network and nothing more (scope,
not implementation status — see §4): fixed supply (one genesis mint of
`SUPPLY = 1_000_000_000_000_000` base units, no minting after genesis per
[ADR-0011](docs/adr/0011-genesis.md)), virtual-only (ledger entries between
pseudonymous node keypairs, no redemption path, no bridge, nothing leaves
the network), and no leverage (no escrow, lending, margin, marketplace, or
autonomous-spending policy — enforced by the gate while `FAL_LEVEL < 3`).
Full placement text: [safety case §2](docs/safety/mvp-safety-case.md#2-placement-the-mvp-is-fal-2).

### 3.2 The contained-blast-radius argument

Fixed supply **plus** virtual-only **plus** no leverage bounds the worst
case to recoverable in-network harm — never external financial loss. The
adversary's ceiling at FAL-2 is ledger corruption, reputation collapse, and
denial of service via social fork, each recoverable by checkpointing and
re-joining; catastrophic external harm requires a bridge or leverage
primitive the gate forbids. The bound holds of the specified FAL-2 design
and becomes an enforced guarantee as the §4 planned safeguards land. Full
argument: [safety case §3](docs/safety/mvp-safety-case.md#3-the-contained-blast-radius-argument).

### 3.3 What FAL-2 excludes (FAL-3 triggers)

The following are **not** in the MVP. While `FAL_LEVEL < 3` the gate fails
the build if any appears anywhere outside allowlisted prose: escrow
(including HTLC and multi-sig escrow), lending and borrowing (loans),
margin and yield, fiat and crypto bridges, compute marketplace,
policy-based autonomous spending. Raising the level requires the pause-rule
evidence in order (see §2). Full trigger list and procedure:
[safety case §4](docs/safety/mvp-safety-case.md#4-what-fal-2-excludes-fal-3-triggers)
and [fal-level.md](docs/safety/fal-level.md).

## 4. Safeguards assessment (summary — statuses live in the safety case)

Every safeguard claim carries exactly one status from the safety-case
legend. This file states the shape; [safety case §5](docs/safety/mvp-safety-case.md#5-safeguard-claims-and-their-evidence)
owns the evidence links and stays the single content pipeline.

Implemented today (§5a): the FAL level is declared in one constant and
pinned by test (S1), no FAL-3 capability can land without failing the build
locally and in CI (S2), node keys are Ed25519 with owner-only persistence
and permission repair (S3), backup/restore is validated-before-write (S4),
and dependency risk is gated by `cargo audit` + `cargo deny` (S5).

Planned — explicitly not done, tracked by ticket (§5b): validated
transfers (P1), gossip/sync validation (P2), fork-choice with equivocation
evidence and checkpoints (P3), signed genesis artifact and recovery runbook
(P4), tenure-weighted reputation with whitewash cost (P5), scoring plus
rate limits plus relay caps (P6), authenticated transport with
domain-separated signatures (P7), the local `cargo test` FAL-2 proof
harness (P8), per-release risk notes (P9), and this document track itself
(P10). No other safeguard is claimed; anything not in §5a is still to
build, and §5b names the ticket that must build it.

Evaluation posture: today the proof is the enforced subset (S1–S5) run in
CI on every PR; the full FAL-2 proof (P8 harness per release plus the
6-month heartbeat, with pause-trigger and coverage/publish dates per
ADR-0006) is planned ([#41](https://github.com/kudosscience/clawbank/issues/41),
[#42](https://github.com/kudosscience/clawbank/issues/42),
[#43](https://github.com/kudosscience/clawbank/issues/43)). Until that
harness lands, the case rests on the blast-radius bound plus the
implemented gates, not on adversarial results that do not yet exist. Full
posture: [safety case §6](docs/safety/mvp-safety-case.md#6-evaluation-posture-what-proves-it).

## 5. Follow-up, deployment outcomes, and governance (Docs 02 owns the operating text)

Capability, safeguards, and follow-up assessment procedures a reviewer can
execute; the pause rule with concrete Continue and Restrict outcomes;
downscaled maintainer-as-RSO governance with PR gating tiers and the
anti-retaliation commitment; and Appendices A–C/E–F (glossary, FAL-2
standard, threshold dossier, reviewers, compliance checklist) land in Docs
02 ([#39](https://github.com/kudosscience/clawbank/issues/39)), which
extends this file in place.

What Docs 01 locks now (imported from Safety 02, not duplicated):

- Safety decision-maker: the maintainer decides the FAL level, merges
  `SAFETY.md` changes, and owns the pause decision; a rise to FAL-3
  additionally requires an external reviewer, recorded here (Appendix C +
  Changelog) and in a per-release Risk Report.
- Non-retaliation: anyone may raise a safety concern — as an issue with
  the `safety` label, as a PR, or as a pause request — without fear of
  retaliation; good-faith false alarms are welcomed; emergency pause
  (`git tag -s pause/YYYY-MM-DD`) needs no prior permission and is
  followed by a `safety`-labelled issue plus the 7-day full report.
- Threshold discipline: the FAL threshold is re-evaluated on every
  safeguard upgrade; any escrow/lending/bridge/autonomous-spending proposal
  is gated by the pause rule until safeguards, harness, and reviewer
  sign-off all land.

Full governance text: [safety case §7](docs/safety/mvp-safety-case.md#7-governance-and-non-retaliation).
Per-release cadence, sign-off, and redaction rules:
[risk-reports.md](docs/safety/risk-reports.md). Pause procedure:
[fal-level.md](docs/safety/fal-level.md).

## Appendix D: Changelog

Every change to this commitment after `v0.1.0` gets a row below (newest
first) with a redline. Redlines are git diffs between signed safety tags
(`git diff safety/vX..safety/vY -- SAFETY.md`) with a GitHub compare URL;
the release gate (`scripts/ci/release-gate.sh`) enforces that the
`Version:` header above equals the tag being cut.

| Version | Date | Change | Redline |
| ------- | ---- | ------ | ------- |
| 0.1.0 | 2026-09-07 | Initial Docs 01 core: preamble, FAL background imported from ADR-0004, threshold/safeguard matrix (non-normative summary), Safety 02 justification core housed by reference, Version/Effective header, and this Changelog discipline. Operating assessment procedures, deployment/governance text, and Appendices A–C/E–F deferred to Docs 02 (#39); signing/tags/attestation deferred to Docs 03 (#40). | Initial version — no prior safety tag; entire file is the change. Verify with `git show safety/v0.1.0:SAFETY.md`. Future rows link `https://github.com/kudosscience/clawbank/compare/safety/vX...safety/vY` |

Verification (reproduces the latest entry once the tag is cut):

```sh
bash scripts/ci/release-gate.sh safety 0.1.0
git show safety/v0.1.0:SAFETY.md | grep -m1 -E '^Version:'
# after the next safety release vX.Y.Z:
# git diff safety/v0.1.0..safety/vX.Y.Z -- SAFETY.md
```

# MVP safety case: why FAL-2 is safe

> **Pipeline note (read first).** This file is the argument track for
> issue [#36](https://github.com/kudosscience/clawbank/issues/36)
> (Safety 02). It is written as the future content of the `SAFETY.md`
> justification section: capability assessment, safeguards assessment,
> and follow-up for the MVP. The document skeleton, signing, and
> attestation belong to the ADR-0005 track —
> [#38](https://github.com/kudosscience/clawbank/issues/38) (Docs 01,
> core) and [#39](https://github.com/kudosscience/clawbank/issues/39)
> (Docs 02, assessments/governance/appendices) — which own merging this
> content into repo-root `SAFETY.md` without duplicating or diverging
> from it. Normative level definitions live in
> [ADR-0004](../adr/0004-safety-risk-levels.md); the table in §1 is a
> non-normative summary of it — this file creates no second normative
> copy.
>
> **Status legend for §5.** Every safeguard claim in §5 carries
> exactly one status: **[Implemented]** (shipped code, linked below),
> **[Planned — #N]** (open ticket, explicitly not done), or
> **[Procedural]** (human/CI process, linked to the procedure). Nothing
> in this file implies an unimplemented safeguard is done.
> Design-scope statements in §§1–4 describe what FAL-2 *requires* per
> ADR-0004, not what is implemented today: implementation status lives
> in §5, and the §§2–3 argument is conditional on the §5b safeguards
> landing (see §6 for what is enforced today).

## 1. The FAL-1..4 scale (imported from ADR-0004)

| Level | Name | What agents may do | Required safeguards (summary) |
| ----- | ---- | ------------------ | ----------------------------- |
| FAL-1 | Sandbox | Single-node/LAN, non-transferable or trivial balances, no P2P/reputation | Local `0o600` key storage only |
| FAL-2 | Virtual credit network (**MVP — ship here**) | Fixed-supply virtual credits, genesis mint only, P2P transfers, replicated ledger, history-derived reputation | Typed validated transfers, rate limits + message scoring, tenure-weighted reputation with whitewash cost, explicit fork-choice + social-fork checkpoint, authenticated transport, relay caps, `cargo audit`/`deny` |
| FAL-3 | Real-value-adjacent | Escrow (incl. HTLC/multi-sig), lending/borrowing, margin/yield, fiat/crypto bridges, compute marketplace, policy-based autonomous spending | FAL-2 bundle **plus** defense-in-depth (spending caps/allow-lists, inline anomaly classifier, async ledger analysis, pause/freeze rapid response) **plus** the 17-control subset (multi-party mint/bridge auth, SLSA provenance, audit log/SIEM, honeypot escrow, external red-team), Risk Reports + external reviewer |
| FAL-4 | Autonomous macro-economy | Recursive credit→compute earn loops persisting without human re-auth, systemic external-market impact | Undefined today; committed to define before reaching FAL-3 |

Source of truth: [ADR-0004](../adr/0004-safety-risk-levels.md).
The "Required safeguards" column above states what each level *requires*,
not what is implemented today: of the FAL-2 bundle, only key storage
(S3), the FAL pin + gate (S1–S2), and `cargo audit`/`deny` (S5) are
**[Implemented]** — the rest is **[Planned]** (see §5).
Mechanical declaration: `clawbank_safety::FAL_LEVEL`
([`crates/clawbank-safety/src/lib.rs`](../../crates/clawbank-safety/src/lib.rs),
currently `2`), pinned by the gate test
([`crates/clawbank-safety/tests/fal_gate.rs`](../../crates/clawbank-safety/tests/fal_gate.rs)).
Operator procedure: [`fal-level.md`](fal-level.md).

## 2. Placement: the MVP is FAL-2

The MVP's design scope is a virtual credit network and nothing more
(scope, not implementation status — see §5):

- **Fixed supply.** One genesis mint creates the entire supply
  (`SUPPLY = 1_000_000_000_000_000` base units = 1B credits); no minting
  exists after genesis
  ([ADR-0011](../adr/0011-genesis.md)). **[Planned — #54](https://github.com/kudosscience/clawbank/issues/54)**
  builds the artifact, verification, and strict boot; the supply number
  and allocation rule are already locked by ADR-0011.
- **Virtual-only.** Credits are ledger entries between pseudonymous
  node keypairs. There is no redemption path, no fiat or crypto bridge,
  no convertibility promise — nothing leaves the network. (Absence of
  bridge/escrow/lending code enforced by [S2](#5a-implemented-today).)
- **No leverage.** No escrow, no lending/borrowing, no margin/yield,
  no marketplace, no autonomous-spending policy. Any of these in code
  trips the pause-rule gate while `FAL_LEVEL < 3`
  (**[Implemented]**
  [`crates/clawbank-safety/tests/fal_gate.rs`](../../crates/clawbank-safety/tests/fal_gate.rs)).

## 3. The contained-blast-radius argument

Fixed supply **plus** virtual-only **plus** no leverage bounds the worst
case to **recoverable in-network harm — never external financial loss**.
The bound below holds of the specified FAL-2 *design*; it becomes an
enforced guarantee only as the §5b safeguards land — today the enforced
subset is §5a (see §6):

1. **No external financial loss is possible.** With no bridge and no
   redemption, there is no channel by which a ledger event becomes a
   real-world monetary loss. An attacker can corrupt balances or
   reputation *inside* the network; they cannot extract value *out* of
   it, because there is no out.
2. **Ledger corruption is recoverable.** The worst ledger outcome —
   conflicting histories, inflated or destroyed balances — is answered
   by the social fork: pause, pin a signed checkpoint, and re-join under
   a checkpoint or a new genesis artifact. It costs time and trust, not
   funds (see `Checkpoint` / `Social fork` in
   [`CONTEXT.md`](../../CONTEXT.md), recovery runbook
   **[Planned — #55](https://github.com/kudosscience/clawbank/issues/55)**,
   fork-choice/evidence design in [ADR-0009](../adr/0009-ledger-replication.md)
   whose implementation is **[Planned — #51](https://github.com/kudosscience/clawbank/issues/51)**).
3. **Reputation collapse is recoverable.** Reputation is display and
   routing guidance only ([`CONTEXT.md`](../../CONTEXT.md)); a gaming or
   Sybil attack degrades signal quality but cannot move real value.
   Recovery is parameter retuning plus checkpointed history, not a
   bailout (scorer **[Planned — #52](https://github.com/kudosscience/clawbank/issues/52)**,
   serving/routing **[Planned — #53](https://github.com/kudosscience/clawbank/issues/53)**,
   adversarial driver **[Planned — #42](https://github.com/kudosscience/clawbank/issues/42)**).
4. **Denial of service ends at the social fork.** Dust floods, gossip
   spam, and partition games can halt or split the network; relay caps,
   scoring, and rate limits (**Planned**:
   [#28](https://github.com/kudosscience/clawbank/issues/28) /
   [#27](https://github.com/kudosscience/clawbank/issues/27) /
   [#42](https://github.com/kudosscience/clawbank/issues/42), row P6 in
   §5b) are designed
   to bound the damage, and a split community
   re-joins under a new checkpoint. Availability is at stake, solvency
   is not.

In short: the adversary's ceiling at FAL-2 is *ledger corruption,
reputation collapse, denial of service via social fork* — each
recoverable by checkpointing and re-joining. Catastrophic external harm
requires a bridge or leverage primitive, and those are FAL-3 capabilities
the gate forbids.

## 4. What FAL-2 excludes (FAL-3 triggers)

The following are **not** in the MVP. While `FAL_LEVEL < 3` the gate
fails the build if any appears anywhere outside allowlisted prose —
**[Implemented]**
([gate](../../crates/clawbank-safety/tests/fal_gate.rs),
[procedure](fal-level.md)):

- escrow (including HTLC and multi-sig escrow)
- lending and borrowing (loans)
- margin and yield
- fiat and crypto bridges
- compute marketplace
- policy-based autonomous spending

Raising the level requires the pause-rule evidence *in order* —
safeguards (ADR-0004 FAL-3 bundle + 17-control subset), evaluations
(ADR-0006 FAL-3 harness), reviewer (maintainer + external, recorded in
`SAFETY.md`) — never a bare constant flip. The constant, the gate, and
the procedure are linked in §1.

## 5. Safeguard claims and their evidence

### 5a. Implemented today

| # | Safeguard claim | Evidence |
| - | --------------- | -------- |
| S1 | The FAL level is declared in exactly one constant (`FAL_LEVEL = 2`) and pinned by test | **[Implemented]** [`crates/clawbank-safety/src/lib.rs`](../../crates/clawbank-safety/src/lib.rs), [`crates/clawbank-safety/tests/fal_gate.rs`](../../crates/clawbank-safety/tests/fal_gate.rs) (`fal_level_pins_mvp_at_two`) |
| S2 | No FAL-3 capability can land in the tree without failing the build, locally and in CI | **[Implemented]** [`crates/clawbank-safety/tests/fal_gate.rs`](../../crates/clawbank-safety/tests/fal_gate.rs) (`no_fal3_capabilities_below_fal3`), run via `scripts/ci/rust-check.sh test` in the `rust-test` CI job ([`scripts/ci/rust-check.sh`](../../scripts/ci/rust-check.sh), [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)) |
| S3 | Node keys are Ed25519 generated from the OS random source, persisted owner-only (`0o600` file / `0o700` dirs), with concurrent-init safety and permission repair | **[Implemented]** [`crates/clawbank-identity/src/lib.rs`](../../crates/clawbank-identity/src/lib.rs), [`crates/clawbank-identity/src/fs_secure.rs`](../../crates/clawbank-identity/src/fs_secure.rs), [`crates/clawbank-identity/src/paths.rs`](../../crates/clawbank-identity/src/paths.rs), [`crates/clawbank-identity/src/peer.rs`](../../crates/clawbank-identity/src/peer.rs) |
| S4 | Identity backup/restore is validated-before-write and never widens access | **[Implemented]** `export`/`import` in [`crates/clawbank-identity/src/lib.rs`](../../crates/clawbank-identity/src/lib.rs), CLI surface in [`crates/clawbank/src/main.rs`](../../crates/clawbank/src/main.rs) |
| S5 | Dependency and advisory risk is gated in CI (`cargo audit` + `cargo deny`) | **[Implemented]** [`deny.toml`](../../deny.toml), [`scripts/ci/rust-check.sh`](../../scripts/ci/rust-check.sh) (`audit` shard), `rust-audit` job in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) |

### 5b. Planned — explicitly not done, tracked by ticket

| # | Safeguard claim | Owner |
| - | --------------- | ----- |
| P1 | Transfers are typed and validated: dust threshold, per-sender nonce (replay rejection), Ed25519 domain-separated signature, supply invariant (`u64` amounts, `u128` accumulator) | **[Planned — #44](https://github.com/kudosscience/clawbank/issues/44)** (codec/signing bytes), **[Planned — #45](https://github.com/kudosscience/clawbank/issues/45)** (persist + validate/apply), **[Planned — #30](https://github.com/kudosscience/clawbank/issues/30)** (submission path) |
| P2 | Gossip broadcast validates before forwarding; sync catches up deterministically from seq 0 | **[Planned — #49](https://github.com/kudosscience/clawbank/issues/49)** (broadcast), **[Planned — #50](https://github.com/kudosscience/clawbank/issues/50)** (sync), **[Planned — #28](https://github.com/kudosscience/clawbank/issues/28)** (pipes + validation gate) |
| P3 | Forks resolve by deterministic longest-valid-history fork-choice; equivocation produces gossiped evidence; signed checkpoints pin recovery anchors | **[Planned — #51](https://github.com/kudosscience/clawbank/issues/51)** |
| P4 | Genesis is a maintainer-signed artifact with hash-pin strict boot; join order and three-tier recovery (pause tag → checkpoint → new genesis) are drilled | **[Planned — #54](https://github.com/kudosscience/clawbank/issues/54)** (artifact/verify/boot), **[Planned — #55](https://github.com/kudosscience/clawbank/issues/55)** (join + recovery runbook) |
| P5 | Reputation is tenure-weighted with decay + diversity, and whitewash costs: a new PeerId starts at zero reputation | **[Planned — #52](https://github.com/kudosscience/clawbank/issues/52)** (scorer), **[Planned — #53](https://github.com/kudosscience/clawbank/issues/53)** (serving/routing) |
| P6 | Gossipsub peer scoring + per-peer rate limits + relay `Limit{duration,data}` bound dust/DoS floods | **[Planned — #28](https://github.com/kudosscience/clawbank/issues/28)** (pipes/scoring), **[Planned — #27](https://github.com/kudosscience/clawbank/issues/27)** (relay caps), **[Planned — #42](https://github.com/kudosscience/clawbank/issues/42)** (adversarial driver incl. dust flood) |
| P7 | Transport proves PeerId node-to-node (Noise handshake); messages carry domain-separated signatures | **[Planned — #25](https://github.com/kudosscience/clawbank/issues/25)** (swarm/Noise), **[Planned — #24](https://github.com/kudosscience/clawbank/issues/24)** (handshake authenticity), **[Planned — #23](https://github.com/kudosscience/clawbank/issues/23)** (message signing) |
| P8 | FAL-2 safety is proved by a local `cargo test` harness: transaction integrity (double-spend, replay, supply, fork-choice), Sybil N=50, circular-trade/whitewash, dust flood vs caps, plus pause-trigger | **[Planned — #41](https://github.com/kudosscience/clawbank/issues/41)** (integrity suite), **[Planned — #42](https://github.com/kudosscience/clawbank/issues/42)** (adversarial driver), **[Planned — #43](https://github.com/kudosscience/clawbank/issues/43)** (cadence/evidence/pause drill) |
| P9 | Every FAL-relevant release carries a filled risk note (evaluations run, FAL verdict, coverage vs publish dates, commit, known gaps, redactions marked); raw exploit detail and peer-identifying data are forbidden in the note | **[Planned — #37](https://github.com/kudosscience/clawbank/issues/37)** |
| P10 | `SAFETY.md` skeleton, signing, attestation, and appendices (this file's merge target) | **[Planned — #38](https://github.com/kudosscience/clawbank/issues/38)** (core), **[Planned — #39](https://github.com/kudosscience/clawbank/issues/39)** (assessments/governance/appendices), **[Planned — #40](https://github.com/kudosscience/clawbank/issues/40)** (attestation live) |

No other safeguard is claimed. Anything not in §5a is, by construction,
still to build — and §5b names the ticket that must build it.

## 6. Evaluation posture (what proves it)

Today the proof is the enforced subset: the FAL pin + pause-rule gate
(S1–S2), the identity suite (S3–S4), and the audit gate (S5) all run in
CI on every PR. The full FAL-2 proof — the §5b harness (P8) run
per-release plus the 6-month heartbeat, with pause-trigger and
coverage/publish dates per [ADR-0006](../adr/0006-safety-evaluation-framework.md)
— is **[Planned](https://github.com/kudosscience/clawbank/issues/41)** ([integrity #41](https://github.com/kudosscience/clawbank/issues/41) / [adversarial #42](https://github.com/kudosscience/clawbank/issues/42) / [cadence #43](https://github.com/kudosscience/clawbank/issues/43)).
Until that harness lands, this safety case rests on the blast-radius
bound (§3) plus the implemented gates (§5a), not on adversarial test
results that do not yet exist.

## 7. Governance and non-retaliation

- **Safety decision-maker.** The maintainer —
  [@kudosscience](https://github.com/kudosscience) (Henry Ward), default
  owner for everything per [`CODEOWNERS`](../../.github/CODEOWNERS) —
  decides the FAL level, merges `SAFETY.md` changes, and owns the pause
  decision, per [ADR-0004](../adr/0004-safety-risk-levels.md) and
  [ADR-0005](../adr/0005-safety-doc-pattern.md). A rise to FAL-3
  additionally requires an external reviewer, recorded in `SAFETY.md`
   (Appendix E + Changelog) and a per-release Risk Report.
- **Non-retaliation commitment.** Anyone may raise a safety concern —
  as an issue with the `safety` label, as a PR, or as a pause request —
  without fear of retaliation, exclusion, or reputational penalty. Good-
  faith false alarms are welcomed: the cost of a needless pause is
   accepted as the price of never missing a real one. Emergency pause
   (`git tag -s pause/YYYY-MM-DD`) needs no prior permission; per
   ADR-0006 it is followed by a `safety`-labelled issue plus the 7-day
   full report
   ([ADR-0006](../adr/0006-safety-evaluation-framework.md)).
- **Threshold discipline.** The FAL threshold is re-evaluated on every
  safeguard upgrade; any `escrow`/`lending`/`bridge`/autonomous-spending
  proposal is gated by the pause rule until the FAL-3 safeguards,
  harness, and reviewer sign-off all land.

## 8. Review and approval

- [ ] Maintainer review and approval as the public safety position
      (acceptance criterion of
      [#36](https://github.com/kudosscience/clawbank/issues/36)).
- Merge target: Docs 01 track
  ([#38](https://github.com/kudosscience/clawbank/issues/38)) folds
  §§1–7 into repo-root `SAFETY.md` as its justification core
  (Capability Assessment / Safeguards Assessment / Follow-up),
  importing — not duplicating — ADR-0004 thresholds and this file's
  argument.

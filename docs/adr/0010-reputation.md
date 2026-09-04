# ADR 0010: FAL-2 reputation is a composite score (dust-filtered volume + diversity + tenure, 30d decay) for display and routing only

FAL-2 reputation = composite of (a) volume at or above `DUST_THRESHOLD=100`, (b) distinct counterparties with identical-behavior-across-N-peers flagged, (c) tenure gate (minimum age + minimum 3 distinct peers before non-zero), minus (d) invalid-gossip (P₄) penalties; exponential 30-day half-life on inactivity, new PeerId starts at zero, proven equivocation slashes to zero. Score is display (`alias (short PeerId)` + score in API) plus gossipsub P₅ routing hint — never fork-choice at FAL-2. Consensus params pinned in `genesis.json`/`META`; display weights local. Sybil ships zero-cost with documented limitation at FAL-2 — see `research/safety-risk-assessment` harm #3 and decision record.

## Status

Accepted — implements wayfinder ticket [#11 Reputation computation: metrics, decay, and thresholds](https://github.com/kudosscience/clawbank/issues/11) (grilling, all recommendations accepted). Depends on ADRs 0001/0002/0004/0006/0007/0009.

## Context

ADR 0004 fixes reputation *properties* (tenure-weighted decay, diversity, whitewash cost) and ADR 0006 fixes the *harness* (Sybil N=50, circular-trade/whitewash spot-checks), but not numbers or weights. MVP has no escrow/dispute oracle, so dispute-based success rate is unmeasurable; raw volume is farmable via tiny transfers (Halborn 2026, MDPI 16(14):6929); new keypairs are free (Douceur 2002), so tenure waiting cost is the only FAL-2 Sybil damper. ADR 0009 keeps fork-choice reputation-free to avoid the ledger→reputation→fork-choice loop.

## Considered Options

- **Composite + decay + display-only (chosen)** — Volume counts only `≥DUST_THRESHOLD=100` (reuses ADR 0007 constant, one consensus); diversity = distinct-counterparty count with circular/identical-behavior flag; tenure gate = minimum age + ≥3 distinct peers before non-zero; P₄ invalid gossip → epoch score floor, P₅ maps local score for mesh preference. Decay `score *= 0.5^(idle_days/30)`; whitewash = zero start (self-penalizing); equivocation proof (`/ai-bank/evidence/1.0.0`, ADR 0009) → slash to zero. Reputation-weighted fork-choice (`tip_seq + α·sum_reputation`) stays deferred to FAL-3 behind the ADR 0009 flag + pause gate.
- **Dispute/win-rate metric (rejected)** — no oracle at FAL-2 without escrow; absence-of-payment is indistinguishable from no-transaction.
- **Raw volume / no tenure gate (rejected)** — farmable at machine speed by one operator → 1000 agents; Sybil N=50 harness would fail by design.
- **Reputation-weighted fork-choice now (rejected)** — circular dependency the harness is built to catch; reserved for FAL-3.
- **Invite/PoP/stake Sybil gate at FAL-2 (rejected)** — overkill for contained blast radius (ADR 0004: worst case is social-fork recovery); deferred to FAL-3 pause gate with documented "reputation diluted, distribution gameable" limitation.

## Consequences

- Consensus params (half-life 30d, dust reuse, 3-peer minimum, slash rule) live in `genesis.json`/`META` beside ADR 0007 constants; changing them = soft fork → checkpoint + `SAFETY.md` Changelog + harness re-run (ADRs 0005/0006). Display scaling tunable per node, no consensus impact.
- Reads `redb` via `begin_read` without touching fork-choice (ADR 0007 handoff); feeds P₅ and API only.
- Harness (ADR 0006) asserts: circular trade doesn't inflate past flag, whitewash restarts at zero, 30d-idle decays by half, P₄ floor triggers.
- FAL-3 trigger unchanged: any escrow/lending/bridge/autonomous-spending proposal reopens Sybil-cost (PoP/invite/stake) choice under pause rule.

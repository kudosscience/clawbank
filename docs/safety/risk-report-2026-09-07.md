# Risk report: 2026-09-07 (current tree, FAL-2 reaffirmed)

> Filled example note for the present tree, per
> [`risk-reports.md`](risk-reports.md) and the shape in
> [`risk-report-template.md`](risk-report-template.md). It demonstrates a
> release note the gate accepts; deleting this file (leaving only the
> template) makes `scripts/ci/release-gate.sh` fail loudly.

| Field | Value |
| ----- | ----- |
| FAL | `2` — reaffirmed |
| coverage_date | `2026-09-07` |
| published | `2026-09-07` |
| commit | `45eba26acfecb632701d05df45c05f6b37460767` |

## Evaluations run

All suites below were executed locally on 2026-09-07 against the
evaluated commit and passed:

- `cargo test --all`: green — 13/13 FAL pause-rule gate tests
  (`crates/clawbank-safety/tests/fal_gate.rs`, including
  `fal_level_pins_mvp_at_two` and `no_fal3_capabilities_below_fal3`),
  17/17 `clawbank-identity` unit tests, 8/8 CLI export/import tests,
  5/5 CLI init tests, 0 doc-test failures.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets -- -D warnings`: clean.
- `cargo audit`: clean (1239 advisories loaded, 95 crate dependencies
  scanned, no vulnerabilities).
- `cargo deny check advisories bans licenses sources`: ok on all four
  dimensions.
- FAL-2 adversarial harness (`safety_fal2`: transaction integrity,
  Sybil N=50, circular-trade/whitewash, dust flood vs relay caps):
  **not yet run — harness not yet landed** (see Known gaps). No
  adversarial result is claimed in this note.

## FAL verdict

**FAL-2 reaffirmed.** The tree declares `clawbank_safety::FAL_LEVEL = 2`
(pinned by test) and the pause-rule gate is green: no FAL-3-gated
capability — escrow, lending/borrowing, margin/yield, fiat/crypto
bridges, compute marketplace, policy-based autonomous spending — is
present anywhere outside the allowlisted safety prose. The worst case
remains the contained FAL-2 bound of
[`mvp-safety-case.md`](mvp-safety-case.md) §3 (recoverable ledger
corruption, reputation collapse, denial of service via social fork —
never external financial loss). No escalation is triggered.

## Coverage and publication

- `coverage_date`: `2026-09-07` — the evaluation outcomes above describe
  the tree as of this date.
- `published`: `2026-09-07` — the day this note was written. The two
  dates coincide here because the suites were run the same day; future
  notes keep them distinct whenever a release is cut after its coverage
  window, re-running the suite first.

## Commit

- Evaluated commit: `45eba26acfecb632701d05df45c05f6b37460767`
  (`Safety 02: published MVP safety case`, the HEAD at evaluation
  time). This verdict describes exactly this tree.

## Known gaps

Per §5b of [`mvp-safety-case.md`](mvp-safety-case.md), only the §5a
subset is enforced today (FAL pin + pause-rule gate, identity
lifecycle, audit/deny gates). Everything else is explicitly planned,
tracked, and not claimed:

- P1–P2 transfers/gossip validation, P3 fork-choice/evidence,
  P4 genesis artifact and recovery runbook, P5 reputation scorer,
  P6 rate limits and relay caps, P7 transport authenticity.
- P8 full FAL-2 proof harness: integrity suite, adversarial driver, and
  cadence/evidence/pause drill are not yet built — this note rests on
  the blast-radius bound plus the implemented gates, not on
  adversarial results that do not yet exist.
- P9 (this track): template and procedure land here; the filled note is
  the present file.
- P10 `SAFETY.md` skeleton, signing, and appendices are pending their
  own track and are not claimed here.

## Redactions

None — no material was withheld from this note.

## Attestation

On release, the Sigstore/Rekor bundle carries only the artifact digest
plus the summary verdict of this note (pass, FAL-2, evaluated commit).
This note contains no raw exploit detail and no peer-identifying data,
so there is nothing to strip before attestation: digests plus summaries
only.

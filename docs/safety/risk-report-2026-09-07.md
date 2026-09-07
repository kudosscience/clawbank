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

All executed suites below were run locally on 2026-09-07 against the
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

- P1–P2 transfers/gossip validation
  ([#44](https://github.com/kudosscience/clawbank/issues/44),
  [#45](https://github.com/kudosscience/clawbank/issues/45),
  [#30](https://github.com/kudosscience/clawbank/issues/30),
  [#49](https://github.com/kudosscience/clawbank/issues/49),
  [#50](https://github.com/kudosscience/clawbank/issues/50),
  [#28](https://github.com/kudosscience/clawbank/issues/28)),
  P3 fork-choice/evidence
  ([#51](https://github.com/kudosscience/clawbank/issues/51)),
  P4 genesis artifact and recovery runbook
  ([#54](https://github.com/kudosscience/clawbank/issues/54),
  [#55](https://github.com/kudosscience/clawbank/issues/55)),
  P5 reputation scorer
  ([#52](https://github.com/kudosscience/clawbank/issues/52),
  [#53](https://github.com/kudosscience/clawbank/issues/53)),
  P6 rate limits and relay caps
  ([#28](https://github.com/kudosscience/clawbank/issues/28),
  [#27](https://github.com/kudosscience/clawbank/issues/27),
  [#42](https://github.com/kudosscience/clawbank/issues/42)),
  P7 transport authenticity
  ([#25](https://github.com/kudosscience/clawbank/issues/25),
  [#24](https://github.com/kudosscience/clawbank/issues/24),
  [#23](https://github.com/kudosscience/clawbank/issues/23)).
- P8 full FAL-2 proof harness: integrity suite
  ([#41](https://github.com/kudosscience/clawbank/issues/41)),
  adversarial driver
  ([#42](https://github.com/kudosscience/clawbank/issues/42)), and
  cadence/evidence/pause drill
  ([#43](https://github.com/kudosscience/clawbank/issues/43)) are not yet
  built — this note rests on the blast-radius bound plus the
  implemented gates, not on adversarial results that do not yet exist.
- P9 (this track,
  [#37](https://github.com/kudosscience/clawbank/issues/37)): template and
  procedure land here; the filled note is the present file.
- P10 `SAFETY.md` skeleton, signing, and appendices are pending their
  own track
  ([#38](https://github.com/kudosscience/clawbank/issues/38),
  [#39](https://github.com/kudosscience/clawbank/issues/39),
  [#40](https://github.com/kudosscience/clawbank/issues/40)) and are not
  claimed here.

## Redactions

None — no material was withheld from this note.

## Attestation

On release, the workflow's `actions/attest-build-provenance` step creates
a Sigstore/Rekor-backed provenance attestation for each file matched by
`dist/*`, identifying each subject by its artifact digest. The
attestation does not embed this note's summary verdict — consult this
note separately for the pass/fail outcome, the FAL-2 verdict, and the
evaluated commit. This note contains no raw exploit detail and no
peer-identifying data, so nothing in it needs stripping before it ships
inside the attested docs bundle.

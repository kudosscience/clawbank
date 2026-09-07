# Risk report template (per-release risk note)

> Copy this file to `docs/safety/risk-report-YYYY-MM-DD.md` and fill every
> section. The release gate (`scripts/ci/release-gate.sh`, code releases)
> excludes `*template*` files and validates each filled note: all six
> `##` sections below must be present, `coverage_date` / `published` /
> commit SHA must be named, the Redactions section must contain a
> `[REDACTED]` marker or an explicit `None` (never silently omitted), and
> peer-identifying or secret material must be absent. Procedure, cadence,
> and sign-off live in [`risk-reports.md`](risk-reports.md).

| Field | Value |
| ----- | ----- |
| FAL | `2` (reaffirmed or escalated — see FAL verdict) |
| coverage_date | `YYYY-MM-DD` (last day the evaluations describe) |
| published | `YYYY-MM-DD` (day this note was written) |
| commit | `<full 40-char commit SHA evaluated>` |

## Evaluations run

List every evaluation executed for this release, with command plus
outcome (pass/fail). At FAL-2 the suite is the local `cargo test` harness
per [ADR-0006](../adr/0006-safety-evaluation-framework.md): transaction
integrity (double-spend, replay/nonce, supply invariant, fork-choice),
Sybil N=50 spot-check, circular-trade/whitewash checks, dust-flood vs
relay caps, the pause-trigger test, plus `cargo audit` / `cargo deny`.
Name what was NOT run as well as what was — a missing harness run is a
known gap, not an omission.

- [ ] `cargo test --all` (or the `rust-check.sh test` shard): outcome
- [ ] `cargo audit` + `cargo deny check`: outcome
- [ ] FAL-2 harness suites (`safety_fal2` / integrity / adversarial): outcome or "not yet landed — see Known gaps"

## FAL verdict

State the affirmed level and why. Either **FAL-2 reaffirmed** (no
FAL-3-gated capability — escrow, lending/borrowing, margin/yield,
fiat/crypto bridges, compute marketplace, policy-based autonomous
spending — present in the tree; the pause-rule gate in
`crates/clawbank-safety/tests/fal_gate.rs` is green) or **escalation to
FAL-3 required** (name the triggering capability and halt the release
until the ADR-0004 FAL-3 safeguards, the ADR-0006 FAL-3 harness result,
and maintainer-plus-external-reviewer sign-off all land, recorded in
`SAFETY.md`).

## Coverage and publication

- `coverage_date`: `YYYY-MM-DD` — the evaluations describe the tree as of
  this date. Evaluation results go stale: a release cut later than the
  coverage window must re-run the suite and amend this note.
- `published`: `YYYY-MM-DD` — the day this note was written. Coverage and
  publication dates are intentionally distinct (RSP v3.4 practice per
  ADR-0005): the gap between them is visible, never hidden.

## Commit

- Evaluated commit: `<full 40-char SHA>` (`git rev-parse HEAD` at
  evaluation time). Short SHAs are not accepted: the note must pin the
  exact tree the verdict describes.

## Known gaps

Enumerate every safeguard or evaluation the verdict depends on that is
not yet implemented, each with its tracking ticket. Cross-reference §5b
of [`mvp-safety-case.md`](mvp-safety-case.md) (status legend:
**[Implemented]** / **[Planned — #N]** / **[Procedural]**). A gap the
reader must discover by other means is a defect in this note.

## Redactions

Either list each redaction as `[REDACTED — category — N tokens
withheld]` (one marker per withheld item, category named, e.g.
exploit-method detail or peer-identifying data) or state explicitly:

`None — no material was withheld from this note.`

An empty section fails the gate: redactions are marked, never silently
omitted.

## Attestation

The release attestation (Sigstore/Rekor bundle, see
`.github/workflows/release.yml`) carries only the artifact digest plus
the summary verdict from this note — pass/fail, FAL level, commit. Raw
exploit detail and peer-identifying data are forbidden in the note by
rule (see `risk-reports.md`) and never enter attested artifacts: digests
plus summaries only.

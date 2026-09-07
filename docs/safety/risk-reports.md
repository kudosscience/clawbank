# Risk reports: when the note is written and who signs it off

Every release carries a short, filled risk note — what was evaluated, the
FAL verdict, coverage versus publication dates, the evaluated commit,
known gaps with explicit redaction markers — per
[ADR-0005](../adr/0005-safety-doc-pattern.md) and
[ADR-0006](../adr/0006-safety-evaluation-framework.md). This file states
the procedure; the fixed shape lives in
[`risk-report-template.md`](risk-report-template.md); the release gate
(`scripts/ci/release-gate.sh`) enforces it.

## When a note is written

- **Every FAL-relevant release.** Any release touching `crates/*` or
  transfer/reputation/ledger logic ships with a fresh filled note at
  `docs/safety/risk-report-YYYY-MM-DD.md` (dated by coverage end). Copy
  the template, never edit it in place — the template file itself is
  excluded from the gate.
- **Heartbeat cadence.** At least every 6 months a note (or a heartbeat
  entry in `docs/safety/updates.md` pointing at one) lands even with no
  release, so coverage never silently drifts (depth over frequency, RSP
  v2.0 3→6-month lesson per ADR-0005).
- **On safeguard upgrades.** Thresholds are re-evaluated whenever
  safeguards change (RSP v2.1 practice): the note records the
  re-evaluation outcome.
- **On pause.** Within 7 days of an emergency `pause/YYYY-MM-DD` tag, a
  full report follows under a `safety`-labelled issue.

## Who signs it off

- **Standard notes** (FAL level unchanged): the maintainer reviews and
  merges the note as a standard change, per the governance in
  [`mvp-safety-case.md`](mvp-safety-case.md) §7.
- **FAL threshold changes**: maintainer **plus** an external reviewer,
  recorded in `SAFETY.md` (Appendix C + Changelog) and the per-release
  note. No single party raises the level alone.
- **FAL-3-gated features** (escrow, lending/borrowing, margin/yield,
  fiat/crypto bridges, compute marketplace, policy-based autonomous
  spending): blocked until the ADR-0004 FAL-3 safeguards, the ADR-0006
  FAL-3 harness result showing no meaningful catastrophic misuse, and
  both reviewers land — never a bare constant flip (see
  [`fal-level.md`](fal-level.md)).
- **Emergency pause** (`git tag -s pause/YYYY-MM-DD`): needs no prior
  permission; it is followed by a `safety`-labelled issue plus the 7-day
  full report. Good-faith false alarms are welcomed — the non-retaliation
  commitment in `mvp-safety-case.md` §7 applies.

## What the note must and must not contain

- **Must:** evaluations run (commands plus outcomes, including what was
  NOT run), FAL verdict (reaffirmed or escalated, with reason),
  `coverage_date` versus `published` dates, the full evaluated commit
  SHA, known gaps each with its tracking ticket, and a Redactions
  section that lists `[REDACTED — category — N tokens withheld]`
  markers or states `None` explicitly. Redactions are marked, never
  silently omitted.
- **Must not:** raw exploit detail (methods, payloads, prompts that
  reproduce an attack) and peer-identifying data (network-layer
  addresses, full node identifiers, secret key material) are forbidden
  in the note by rule. That material stays internal-only; the public
  note carries summaries and digests. The gate rejects notes containing
  network address literals, multiaddr locators, or key blocks.
- **Attestation:** the release workflow (`.github/workflows/release.yml`)
  creates a Sigstore/Rekor-backed provenance attestation for each
  released file, identifying each subject by its artifact digest. The
  attestation does not embed the note's summary verdict — pass/fail, FAL
  level, commit stay in the note, which ships inside the attested
  bundle. Raw exploit detail and peer-identifying data are forbidden in
  the note by rule and therefore never enter attested artifacts.

## How the gate ties in

`scripts/ci/release-gate.sh` (code releases, run by the `gate` job in
`.github/workflows/release.yml`) fails a release loudly when no filled
note exists, when any required section (including Attestation) is
missing, when the dates are not real `YYYY-MM-DD` calendar dates, when
the commit pin is not an isolated full SHA, when Known gaps items lack
tracking tickets, when the Redactions section is empty or silent, or
when forbidden material (network address literals including IPv6,
multiaddr locators, key blocks) is detected. Deleting the filled note
makes the gate fail; the template alone never satisfies it. Freshness is
a maintainer duty on top of the structural checks: cut the release tag
at the evaluated commit (or re-run the suite and write a fresh note) —
reusing a stale note passes the gate's shape checks but fails review.

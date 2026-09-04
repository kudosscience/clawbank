# ADR 0005: Safety documentation is SAFETY.md (RSP skeleton) + per-release Risk Reports, audited via git + Sigstore

AI Bank publishes safety as a single repo-root `SAFETY.md` following Anthropic RSP's 7-section skeleton, with per-release redacted Risk Reports, semantic versioning, and auditability via signed git tags + Sigstore/Rekor transparency log + SLSA provenance. `SAFETY.md` is the durable public commitment; Risk Reports are the evidence; git is the transparency log — see `research/safety-doc-pattern` decision record.

## Status

Accepted — implements wayfinder ticket [#6 Safety documentation pattern: how to publicly justify safety at each level](https://github.com/kudosscience/clawbank/issues/6) → `research/safety-doc-pattern` (`docs/research/safety-doc-pattern.md:1` on `research/safety-doc-pattern`). Depends on ADR 0004 (FAL-1..4).

## Context

Safety is part of the MVP with maintainer (RSO) governance and no board, no cloud bills, and no central authority. Commitments must be forkable, locally verifiable, and survive without a hosted service — analogous to Ethereum Foundation walkaway test and Bitcoin BIPs. Anthropic RSP (v1.0 Sep 2023 → v3.4 Jul 2026) + Seoul Commitments (May 2024) + METR Common Elements provide the grammar; RSP's board/LTBT/RSO machinery does not map.

## Considered Options

- **SAFETY.md at repo root + Risk Reports + git+Rekor (chosen)** — One file `SAFETY.md` (RSP §§1–7 + Appendices A–F + Changelog, 800–1400 lines, semver `Version` + `Effective` date, supersedes pointer): Preamble / Background (FAL model imported from ADR 0004, not duplicated) / Capability Thresholds & Required Safeguards (4-row matrix) / Capability Assessment / Safeguards Assessment / Follow-up / Deployment & Scaling (6.1 Continue / 6.2 Restrict pause rule) / Governance & Transparency, plus Appendices (Glossary, FAL-2 Standard, Detailed Thresholds from ADR 0004, Changelog, Reviewers, Compliance Checklist). Versioning via `git tag -s safety/vMAJOR.MINOR.PATCH` with `git verify-tag` + `git diff safety/vX..safety/vY` redline + compare URL in Changelog (BIP-0003 pattern). Auditability via Sigstore public Rekor: Fulcio OIDC ephemeral cert → SLSA provenance (in-toto, source URI + builder ID + commit SHA) → Rekor entry (SCT + Signed Tree Head, witness cosigs) → bundle with release; verify via `slsa-verifier` / `cosign verify-blob` + `rekor-cli verify`. Public by default, redact by rule with `[REDACTED — category — N tokens withheld]` and coverage/publication dates (`coverage_date` vs `published`, RSP v3.4). Governance downscaled: maintainer merges `SAFETY.md` (standard change), external reviewer required for FAL threshold changes, FAL-3 features blocked without safeguards+harness+reviewer, emergency `pause/<date>` tag within 7 days.
- **Hosted Notion/wiki or per-model System Cards only (rejected)** — not forkable, requires server, not hash-addressed, loses walkaway property.
- **Copy RSP org chart verbatim (rejected)** — board + LTBT + Executive Risk Council assumes org that doesn't exist; overkill for FAL-2, would stall MVP.

## Consequences

- File lives at repo root (not `docs/`), alongside `SECURITY.md`; maintainer pubkey stored at `docs/safety/MAINTAINER_PUBKEYS.asc` / `.github/trusted-keys/`; tag pattern `safety/v*` protected to require signed tags.
- Cadence: every release touching FAL-relevant code → Risk Report; at least every 6 months heartbeat (`docs/safety/updates.md`) even with no release; on safeguard upgrade re-evaluate thresholds (RSP v2.1); on pause publish within 7 days; annually procedural self-review (`cargo audit`/`deny` + SLSA check, third-party only at FAL-3+). Depth over frequency (RSP v2.0 3→6 mo lesson).
- Public/private split table enforced: commitments/assessment commitments/evidence summaries public redacted; raw prompts, exploit PoCs, peer IPs internal only; attestation holds only digest + summary + pass/fail, never secrets.
- Paired with ADR 0004's FAL definitions and #7's evaluation harness doc — `SAFETY.md` imports, not restates, thresholds and harness details; planned FAL-3 safeguards page non-binding.
- Adds `docs/safety/` scaffolding (Risk Reports, updates journal, procedural reviews) and GitHub `safety` label + `rsp@` analogue via Issues.

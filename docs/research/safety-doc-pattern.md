# Safety Documentation Pattern: How to Publicly Justify Safety at Each Level

**Wayfinder Research Ticket #6 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**
**Branch:** `research/safety-doc-pattern` | **Date:** 2026-09-02 | **Author:** Muse Spark (research subagent)
**Status:** Research complete — decision-ready
**Dependency note:** Ticket #6 is **blocked by #5** (risk levels needed). This doc researches **document structure in parallel** and uses placeholder `FAL-1..4` definitions from `research/safety-risk-assessment` (branch `research/safety-risk-assessment`). When #5 finalises FAL wording, `SAFETY.md` should import it verbatim — do not duplicate. This doc does **not** re-decide FAL thresholds.

---

## TL;DR for Decision-Maker

| Question | Answer |
|---|---|
| What to publish at MVP? | A single **`SAFETY.md`** at repo root (RSP analogue) + a lightweight **`Risk Report` note per release** (Seoul Commitments analogue). One file is the durable commitment; one file per release is the evidence. |
| Anthropic RSP skeleton to copy | 7 numbered sections + Appendices + Changelog. Sections: Capability Thresholds → Required Safeguards → Capability Assessment → Safeguards Assessment → Follow-up Assessment → Deployment/Scaling Outcomes → Governance & Transparency. Codify **pause rule**, **evaluation cadence**, and **public commitments to publish**. |
| How to downscale for no-board, maintainer-governed? | Replace Board/LTBT/RSO with **Maintainer + lazily-enlisted external reviewer + public Git log as governance**. Keep the *shape* (proposal → review → decision → public record) but compress the *org chart* to one voice. |
| Public vs internal? | **Public by default, redact by rule** (Anthropic RSP §3.5 / Seoul VII): publish capability thresholds, required safeguards, evaluation *summaries* + pass/fail. Keep raw eval prompts, private keys, exploit PoCs, and peer IPs internal. Adopt **redaction markers** (`[REDACTED — exploit detail]`) so omission is auditable, not silent. |
| Auditability without central authority? | **Signed git tags + Sigstore/Rekor transparency log + SLSA provenance** on every release. Safety commitments become **git-versioned, hash-addressed text**; evaluations become **signed attestations** in Rekor. Anyone can `git verify-tag` + `rekor verify` without trusting a server you run. |
| Update cadence | RSP v2 → 4× effective compute or 6 months; Seoul/RSP v3 → Risk Reports every **3–6 months**. AI Bank MVP: **every release that changes FAL-relevant code** + at least **6-month heartbeat** even if no release (copy RSP's `rsp-updates` page). Record every change in `SAFETY.md` Changelog with semver. |

**Bottom line:** Keep Anthropic's *document shape* and *public-commitment grammar*; strip Anthropic's *organisational weight* and replace with **git as the transparency log**. Your auditability Budget: `git tag -s` is free; Sigstore public Rekor is free; `SAFETY.md` is one file.

---

## 1. Anthropic RSP Structure — Primary Source Anatomy

Primary sources: `anthropic.com/responsible-scaling-policy` hub page + canonical PDFs (v1.0 Sep 19 2023, v2.0 Oct 15 2024, v2.1 Mar 31 2025, v2.2 May 14 2025, v3.0 Feb 24 2026, v3.4 Jul 8 2026) and the `rsp-updates` changelog page. Structure tightened in v2.0; governance and Risk Report machinery added in v3.0.

### 1.1 Section-by-section map (RSP v2.x template)

| § | Heading (RSP v2.x TOC) | What it contains | AI Bank mirror |
|---|---|---|---|
| — | **Preamble / Design Note** | "Proportional, iterative, exportable" framing; Voluntary White House Commitments + Frontier AI Safety Commitments lineage (METR-inspired). Establishes RSP as *voluntary public commitment* not regulation. | `SAFETY.md` preamble: why a decentralised payment net needs an RSP at all. Cite Seoul Commitments + METR framing. |
| 1 | **Background** | Defines **AI Safety Levels (ASL Standards)** as *sets of safeguards*, not model labels (terminology fixed in v2.0, Oct 15 2024). Two current categories: **Deployment Standards** (prevent misuse after release) + **Security Standards** (prevent weight/IP theft). Optional third: internal governance. | Define **Financial Autonomy Levels (FAL)** same way. See §5 of `research/safety-risk-assessment` (FAL = safeguard bundle tied to reversible capability threshold). |
| 2 | **Capability Thresholds and Required Safeguards** | Lists **Capability Thresholds** (CBRN-2, AI R&D-3/4, Autonomy checkpoints; Appendix C) and pairs each with **Required Safeguards** (e.g., ASL-3 Deployment = 4-layer defence-in-depth; ASL-3 Security = 17 controls including multi-party weight auth, SLSA, binary auth, SIEM/SOAR, honeypot weights). Establishes **re-evaluation commitment**: re-visit thresholds whenever safeguards upgrade (v2.1, Mar 31 2025). | FAL threshold table (from #5). Each FAL = threshold description + required safeguards (deployment + security + governance). Explicit **re-evaluation trigger** clause. |
| 3 | **Capability Assessment** (→ 3.1 Preliminary / 3.2 Comprehensive / 3.3 Capability Decision) | How to know a model crossed a threshold. `3.1` quick screen; `3.2` deep eval with **affirmative safety case** (must show model is *sufficiently far* from threshold, not just "pre-defined test passed" — v2.0 change). `3.3` decision gate. Notes elicitation standard: match adversary effort; extend to best-of-N / chain-of-thought; extend eval every **4× effective compute or 6 months post-training** (relaxed from 3 mo in v2.0). | Assessment procedure per FAL (ties to #7): what harness runs, what elicitation level ("adversary-effort equivalent"), what constitutes "affirmative case" that FAL-2 invariant holds. Cadence = every FAL-relevant PR + 6-mo heartbeat. |
| 4 | **Safeguards Assessment** (→ 4.1/4.2/4.3) | Evaluates whether current safeguards *meet* the Required Safeguards for the triggered level. `4.1` Deployment Standard (e.g., real-time prompt+completion classifiers + async monitoring + post-hoc jailbreak rapid response); `4.2` Security Standard (17 controls); `4.3` Safeguards Decision gate. | Evaluate `FAL-2 required safeguards` (§5 of #5) are *actually implemented*: transfer validation, dust threshold, nonce/replay, supply invariant, fork-choice, rate limits, tenure-weighted reputation, relay caps, cargo-audit provenance. |
| 5 | **Follow-Up Capability Assessment** | Checks drift since last assessment (scaling buffer idea: original 6× heuristic, later replaced with informal estimate because "science not mature enough" — v2.0 changelog). | Post-release smoke: supply invariant + Sybil spot-check remain true under new code/params. |
| 6 | **Deployment and Scaling Outcomes** (→ 6.1 Continue / 6.2 Restrict) | **Pause rule**: if safeguards insufficient, *restrict deployment and further scaling* — the "race-to-the-top" incentive. Must not train/deploy until next ASL Standard is met. | **Pause rule verbatim for AI Bank**: "You may not ship features that cross the next FAL's capability threshold unless the next FAL's safeguards + evaluations pass." Ship FAL-2 only; FAL-3 features gated. |
| 7 | **Governance and Transparency** (→ 7.1 Internal / 7.2 External) | `7.1` Internal: **Responsible Scaling Officer (RSO)**, Executive Risk Council (ISO 27001), clearance tiers, noncompliance / anti-retaliation policy (Feb 2026, expanded Mar 24 2026), internal critique. `7.2` External: **board + LTBT approvals** for RSP changes, external reviewer on Capability & Safeguard Reports, **public `rsp-updates` page** (6–12 mo updates, with redlines), U.S. Government notice if ASL-3 required, annual third-party **procedural compliance review** (substantive vs procedural distinction), `rsp@anthropic.com` feedback. | Downscale — see §4. Replace with Maintainer + external reviewer (lazy) + public Git log + `SAFETY.md#changelog`. |
| | **Appendices** | `A: Glossary` (ASL, Capability Threshold, Required Safeguards, Effective Compute…); `B: ASL-2 Standard` (baseline); `C: Detailed Capability Thresholds` (evaluator-readable); `Changelog` (version, date, what changed + redline PDFs). | Same. `Appendix C` = FAL threshold dossier (import from #5). `Changelog` = semver + date + redline via `git diff`. |
| | **ASL-3 Planned Safeguards page** (Oct 15 2024) | Non-binding *future* safeguards preview: the 4-layer deployment stack (access → real-time classifiers → async monitoring → jailbreak rapid response) + 17 security controls. Shows "we will reach this" without overcommitting to dates. | Optional for AI Bank: non-binding `Planned FAL-3 Safeguards` page (escrow, SLSA provenance, anomaly detection) to guide contributors. |

### 1.2 Commitments that make RSP auditable (not just descriptive)

- **Public commitment not to scale without safeguards.** The first sentence of every PDF version: "a public commitment not to train or deploy models capable of causing catastrophic harm unless we have implemented safety and security measures that will keep risks below acceptable levels."
- **Affirmative case, not checklist tick.** v2.0 replaced pre-approved eval lists with "affirmative case that models are sufficiently far from thresholds." Evaluator must argue safety, not just report pass.
- **Pause rule with teeth.** §6.2 explicitly blocks further training/deployment. Creates economic incentive to finish safeguards.
- **Re-evaluation commitment.** v2.1: "reevaluate Capability Thresholds whenever we upgrade to a new set of Required Safeguards."
- **Versioned policy + public redlines.** Every version archived with `effective <date>` and a `redline` PDF diff (§7.2.7: publish updated version *on or before* effective date, record differences in Change Log).
- **Procedural compliance review.** §7.2.4: *annual* third-party review of *whether we followed our own procedure* (not whether outcomes were good) + more frequent internal reviews. Separates "did we lie about following our process?" from "was the process good?"
- **Noncompliance & anti-retaliation.** Standalone Feb 2026 policy, expanded Mar 24 2026: multiple reporting channels + informal inquiry path + alignment with RSP v3. Employees can flag violations without retaliation; LTBT can request external review (v3.2, Apr 29 2026).
- **`rsp-updates` page as transparency log.** Append-only public journal; every governance event lands there, even retrospective fixes to sloppy disclosure (§1.3 "we fell short" reflection in Oct 15 2024). See §6 for AI Bank analogue.

### 1.3 Update cadence evolution (what the changelog teaches)

| Event | Cadence rule |
|---|---|
| v1.0 (Sep 2023) | Evaluations every ~3 months. |
| v2.0 (Oct 15 2024) | Relaxed to **4× Effective Compute *or* 6 months** of post-training enhancements. Rationale: 3-mo forced rushed, lower-quality elicitation. |
| v3.0 (Feb 24 2026) | Introduced **Risk Reports every 3–6 months** (not per model release) + **Frontier Safety Roadmap** (goal board). Plus growing `rsp-updates` discipline: anticipate **6–12 month** public updates at minimum. |
| v3.2–v3.4 (Apr–Jul 2026) | Tuned redaction / reviewer / coverage-date rules: unredacted reports to ≥200 staff, redacted public reports must mark redaction points, multiple reviewers can split sections. Shows pattern of **tightening transparency mechanics without changing core promises**. |

Lesson for AI Bank: cadence relaxations are *features*, not failures — RSP learned that frequent-but-shallow evals are worse than rarer-but-deep ones. Copy that lesson: promise **depth over frequency**.

---

## 2. Other Analogous Public Safety Documents

### 2.1 Seoul Frontier AI Safety Commitments (May 2024; 16 → 20 signatories)

8 paragraphs (I–VIII) mapping to three outcomes; the guide for implementers (*safetyframeworkguide.com*) makes the **public vs internal split** explicit:

| Commitment | Public artefact |
|---|---|
| I–III (identify/assess/manage risks; accountable development) | Published **safety framework** before France AI Summit (deadline: early 2025). |
| VII (transparency) | "Provide public transparency on the implementation of I–VI, except insofar as doing so would increase risk or divulge sensitive commercial information disproportionate to societal benefit. **Still share more detailed information with trusted actors (home government).**" |
| VIII (external involvement) | Explain *how* governments / civil society / academia / public are involved in assessing risks, adequacy, and adherence. |

METR's "Common Elements" study (Aug 2024 → Dec 2025 update) shows the **de facto template** now converged upon across 12 publishers (Anthropic, OpenAI, Google DeepMind, Meta, Microsoft, Amazon, xAI, etc.):
- 9/12 have capability thresholds; 12/12 have deployment mitigations + accountability + update policy; 11/12 have weight security; 9/12 have halt-conditions.

Takeaway: Seoul codified RSP's *grammar* into industry lingua franca. Any `SAFETY.md` that hits those 8 boxes will be legible to policy readers without extra framing.

### 2.2 OpenAI Preparedness Framework & Google DeepMind Frontier Safety Framework

These are the two closest RSP siblings (cited as templates by the Seoul guide).

- **OpenAI PF (beta, Dec 2023):** Uses **Preparedness Levels** (low→critical) across four tracks (cyber, bio, persuasion, autonomous replication). Each level = eval suite + mitigations matrix. Adds **Preparedness Scorecard** + model-specific **System Cards**. Cadence: scorecards per model release.
- **DeepMind FSF (May 2024):** Uses **Critical Capability Levels (CCL)** with **safety cases** per CCL, third-party evals, and a **CC BY-licensed technical report** structure (explicitly exportable).

Commonality with RSP: all three separate **threshold definition** from **mitigation prescription** from **evaluation evidence** — do not fuse them into one narrative. AI Bank `SAFETY.md` should do the same (FAL definitions vs FAL-2 safeguards vs evaluation summaries are three blocks, not interleaved).

### 2.3 Ethereum Foundation Mandate (Mar 13 2026) — stewardship analogue

Ethereum's problem is close to AI Bank's: *decentralised protocol + no central board + must remain forkable + walkaway test* (network must survive even if core team disappears).

Key patterns to borrow:

- **Governance minimisation** as safety: "No social layer should override protocol guarantees lightly." Document *powers that are deliberately not claimed* (EF: "We are NOT an Accreditation Body / Regulator / Government").
- **Public & auditable, forkable by design.** "All work must be public and auditable: no proprietary black boxes. All work must also be forkable." Publish specs + tests + docs that can be reused without EF.
- **Compounding upstream leverage.** Prioritise "shared primitives, specifications, tooling, and evaluation methods that … can be freely reused, extended, and operated independently." AI Bank `SAFETY.md` *is* that upstream artefact.
- **Zero-option / credible constraint.** "For every affordance that has an intermediated path, any intermediary-free path that is possible must be built and must remain credible." For AI Bank: every safety promise must have a *locally verifiable* counterpart (local test + offline Rekor verify), not just a hosted dashboard.

### 2.4 Bitcoin BIPs & BIP-0003 Process (2011→2026) — decentralised standard analogue

BIPs are the longest-running template for **auditable commitments without central authority**.

| BIP mechanism | What it solves | AI Bank reuse |
|---|---|---|
| **BIP = design document** (`Abstract + Motivation + Specification + Rationale + Reference Implementation + Test Vectors`). Status track `Draft → Complete → Deployed`. | Forces rationale to outlive implementation. | `SAFETY.md` mirrors BIP header: Status, Type (Process), Author, Created, Requires (FAL dossier from #5). |
| **BIP Editors are librarians, not authorities.** They check scope + editorial criteria; they do *not* judge quality. BIPs are *published if on-topic*, not *approved if good*. | Separates editorial from gubernatorial power. | Maintainer as `SAFETY.md` editor, not certifier: merges well-formed safety proposals if they meet format; community decides adoption by running code. |
| **Reference implementation + test vectors required before `Complete`.** | Prose without code is not credible. | Each `SAFETY.md` Required Safeguard must link to code + test proving it (see §3). |
| **BIPs repo as transparency log.** The `bitcoin/bips` git repo is the append-only archive; every change is a PR + review. BIPs themselves state "The BIPs repository serves as a publication medium and archive … The BIP process is not intended to be a kind of forceful governance, merely to provide a collaborative repository for proposing and providing information on standards, which people may voluntarily adopt or not." | Git *is* the governance. | AI Bank safety commitments *are* the `SAFETY.md` git history. Forkable, mirrored, signed. No separate governance DB. |
| **Comments page + mailing list rough consensus.** `Draft → Active` for Process BIPs requires 1 month on `bitcoindev` + no unaddressed substantiated objection. Revisions timestamped. | Public dissent is part of the record. | Small version: Discuss FAL bumps as GitHub issues / Discussions; require no unaddressed substantive objection before `SAFETY.md` FAL upgrade. Timestamp via Git. |
| **Changelog with semver (MANDATORY after `Complete`).** Every post-`Complete` change gets `Version / Date / Description` sorted newest-first; `Version` header bumped. | Readers know what's new without re-reading. | Copy verbatim for `SAFETY.md`. |

Also instructive: **BIP-132** (Committee-based acceptance) shows how to make rough-consensus auditable without a board (4 segments × 70% stake, 2+2-week cycles, declarations of 1%+ stake). For AI Bank MVP, that weight is overkill; adapt only its *principle*: define segment + stake + window narrowly, and record dissent plainly.

### 2.5 Sigstore / SLSA / Rekor — supply-chain auditability for releases

AI Bank is Rust + user-run nodes; safety commitments must survive **without hosted verification infrastructure**.

- **Sigstore (Fulcio + Rekor):** Short-lived certificates from OIDC → sign → append to **Rekor append-only transparency log** (Merkle tree, Signed Tree Head, witness cosigs). Verifiers check bundle offline: signature valid + cert issued by trust root + SCT timestamp within cert validity + inclusion proof in log. Compromise is detectable by **monitor** tailing log for your identity (rekor-monitor GitHub Action; omniwitness for consistency).
- **SLSA provenance + npm as template:** `npm provenance` generates in-toto + SLSA predicate, signs via Fulcio/Rekor, verifies via `npm audit signatures` / `slsa-verifier verify-npm-package` with `--source-uri` + `--builder-id` + `--package-version`. Approach ports to Rust crates via `cargo-sigstore` / SLSA GitHub generators for `crates.io` artifacts (binary verification = `cargo audit` + `cargo verify` path).
- **Trust Root via TUF.** Public keys distributed via `sigstore/root-signing` under TUF; clients `tuf-js` the latest root. No long-lived per-release key to lose.
- **Private vs public.** Rekor entries are public. *Sensitive contents* (exploit PoCs, user IPs) must not go in attested predicate — keep raw eval logs internal; attestation holds only *hash + summary + pass/fail*.

Pattern: **Sign the safety-relevant artefact (node binary + `SAFETY.md` + evaluation summary) and put the signature *in* Rekor; keep sensitive material *out* of Rekor.**

---

## 3. What Should Be Public vs Internal (Evaluation Details?)

Rule of thumb from Seoul VII + RSP §3.5 + EF Mandate: **Public by default, redact by rule, mark redactions.** Do not hide that you hid — make omission machine-readable.

### 3.1 Recommended split for AI Bank

| Bucket | Examples | Where it lives | Visibility |
|---|---|---|---|
| **Safety commitments** (normative) | FAL definitions + capability thresholds + Required Safeguards + pause rule + re-evaluation commitment + changelog + governance statement | `SAFETY.md` at repo root (versioned, signed tag) | **Public, never redacted** |
| **Evaluation commitments** (how you test) | Evaluation cadence + harness scope + elicitation standard + what constitutes "affirmative case" + what triggers escalation | `SAFETY.md` § Capability/Safeguards Assessment + `docs/research/safety-evaluation-framework.md` (ticket #7) | **Public** |
| **Evaluation evidence (per-release)** | Risk Report note: what was run, on what code, when, by whom, summary results (pass/fail), known gaps, and [REDACTED] markers | `docs/safety/risk-report-YYYY-MM-DD.md` (lightweight) or `SAFETY.md#decisions-so-far` appendix; linked from `CHANGELOG.md` + GitHub Release | **Public, redacted** |
| **Evaluation detail (sensitive)** | Full prompt suites, jailbreak payloads, Sybil botnet scripts, exploit PoCs, raw ledger snapshots with peer IPs, model weight excerpts | Private repo / local; shared only with maintainer + external reviewer under NDA | **Internal** |
| **Attestation** (bridges public↔internal) | SLSA provenance: `{ artifact digest, source URI, builder ID, commit SHA, attestation: "FAL-2 evals passed @ <coverage date> (redacted)", Rekor entry }` | Signed bundle attached to GitHub Release; also served from `docs/safety/` as link | **Public hash + proof** |
| **Security-sensitive configurables** | Relay allow-lists, private keys, genesis alloc list before reveal, raw telemetry | Never in eval attestation | **Internal, never logged** |

### 3.2 How to redact without losing auditability

Copy RSP v3.4 §3.5 wording pattern verbatim (adapted):

> "Public Risk Reports contain indications of where material was redacted. Redaction is applied per the following rules: (a) exploit-enabling detail, (b) data that would de-anonymize peers, (c) legally privileged incident notes. Redacted sections are replaced with `[REDACTED — category — N tokens withheld]` and counted in the report's metadata."

This matches RSP's shift Jul 8 2026: "requires public Risk Reports to contain indications of where material was redacted" — the marker makes non-disclosure auditable.

### 3.3 Coverage date vs publication date

Copy RSP v3.4 (Jul 8 2026) rule: reports analyse risk *as of a coverage date* (e.g., `coverage: 2026-07-15`), not publication date. Avoids rushed analysis of last-week changes. AI Bank releases should declare both:

```markdown
---
coverage_date: 2026-09-02  # code evaluated
published: 2026-09-03      # report cut
commit: abc123def456
FAL: 2
---
```

---

## 4. Adapting for Decentralised, No-Bills, Maintainer-Governed Project

Constraint from map #1: **no board, lightweight governance, no cloud bills, runs on users' hardware.** RSP's board/LTBT/RSO/executive-risk-council is not copyable — but its *functions* are.

### 4.1 Governance translation table

| RSP function | RSP organ | AI Bank analogue | Why it preserves safety |
|---|---|---|---|
| Approve policy changes | Board (+ LTBT consultation) | **Maintainer** merges PRs to `SAFETY.md`. For FAL-level bumps, require **maintainer + at least one external reviewer** sign-off (see §4.2). No silent self-merge on FAL scope. | Single-threaded but not unilateral at escalation boundaries. |
| Day-to-day ownership | Responsible Scaling Officer | **Maintainer = RSO.** Explicit line in `SAFETY.md`: `RSO: @<maintainer>` + contact (`rsp@…` analogue: GitHub Issues `[safety]` tag). | Named owner, not diffused. |
| Internal critique | Internal critique requirement | **PR review + `reason: safety` label.** Every FAL-relevant PR requires a written critique comment ("what harm does this enable?") before merge. | Text record > org chart. |
| External input | External experts + LTBT | **Lazy external reviewer pool:** enumerated in `SAFETY.md` Appendix ("External reviewers: …"). For FAL-2, consulted on-demand; for any FAL-3 proposal, mandatory. Copy RSP v3.2 "LTBT approves reviewer selection" as "maintainer records reviewer selection in `SAFETY.md` changelog." | Scales cost: no standing retainer at FAL-2. |
| Compliance oversight | Executive Risk Council (ISO 27001) + annual third-party procedural review | **Annual `cargo audit` + SLSA/Sigstore check + procedural self-review** published as `docs/safety/procedural-review-YYYY.md`. Third-party review only for FAL-3+. | Matches procedural vs substantive split: check "did we follow our own steps?" not "is the policy good?" |
| Public notice | U.S. Government notice if ASL-3 required | **Issue #1 Discussion thread + GitHub Release notes + `rsp-updates`-style page** (see §6). | Zero-cost broadcast. |
| Anti-retaliation | Noncompliance Reporting & Anti-Retaliation Policy (Feb 2026) | **Short clause in `SAFETY.md` + `CONTRIBUTING.md`:** "Anyone may file a safety concern as a GitHub issue with `safety` label; reporter identity protected by GitHub's private vulnerability reporting; no contribution will be rejected as retaliation for a good-faith safety flag." | Captures intent without HR infra. |

### 4.2 Decision gates (lightweight PR gating)

Encode in `SAFETY.md` instead of Notion runbook:

| Gate | Trigger | Requirement |
|---|---|---|
| **Standard change** | Any edit to `SAFETY.md` not changing FAL thresholds | Maintainer PR + CI green. Merge. Changelog entry. |
| **FAL threshold change** | Any edit that changes capability thresholds / required safeguards for a FAL, or adds a new FAL | Maintainer PR + **external reviewer approval comment** + updated `Appendix C` dossier + Changelog `MINOR`. No self-merge. |
| **FAL-3 feature (§5)** | PR that adds `escrow` / `lending` / `fiat bridge` / `autonomous spending policy` | **Blocked by pause rule.** PR must include: (a) FAL-3 safeguard implementation, (b) #7 eval harness results, (c) reviewer sign-off. Maintainer alone cannot waive. |
| **Emergency pause** | Evaluator finds pause condition triggered (e.g., Sybil spot-check fails, dust flood exceeds relay caps) | Maintainer may **tag `pause/<date>` without full review**, open issue, and follow up within 7 days with full report. Modelled on RSP's rapid-response + jailbreak patching tier. |

### 4.3 Proposal not prescription — exportable like BIP

End `SAFETY.md` preamble with the BIP/EIP discipline note (adapted):

> This document is a **public commitment**, not a platform policy. The network adopts it only to the extent that node operators run code that satisfies it. Like a BIP, it serves as a "collaborative repository for proposing and providing information on standards, which people may voluntarily adopt or not." Forks remain possible — safety is achieved by *legibility of the fork*, not by central enforcement.

This matters for walkaway-test credence: you are promising legibility, not control.

---

## 5. Recommended Public Safety Document — `SAFETY.md` Shape

### 5.1 Where it lives and how it's versioned

- **File:** `SAFETY.md` at repo root (not nested under `docs/` — same discoverability as `SECURITY.md` / `CODE_OF_CONDUCT.md` on GitHub).
- **Versioning:** Semver-style header `Version: MAJOR.MINOR.PATCH` + `Effective: YYYY-MM-DD`. Changelog Appendix descending (newest first). Every release tag that changes `SAFETY.md` must bump version. This mirrors BIP-0003's mandatory Changelog after `Complete` and RSP's redline discipline.
- **Git discipline:** `SAFETY.md` is the only file whose *effective date* is semantic, not just commit date. Tag releases: `git tag -s safety/vMAJOR.MINOR.PATCH -m "SAFETY.md v… effective …"` (see §6).
- **Size budget:** 800–1400 lines at MVP (RSP v2.x is ~17 pages; lean by cutting duplicate deployment detail into `#7` harness doc). Anything longer drifts to an appendix file (`docs/safety/appendices/*.md` linked from `SAFETY.md`, not inlined).

### 5.2 Section outline — copy-paste skeleton

Copy this TOC verbatim into `SAFETY.md` and fill it from #5 + #7:

```markdown
# AI Bank Safety Policy (ABSP) — v0.1.0 — Effective 2026-09-02

> Version: 0.1.0 | Effective: 2026-09-02 | Supersedes: (none) | Branch: main | Maintainer/RSO: @<handle> | Feedback: GitHub Issues `safety` label

Preamble — Why a payment network needs an RSP-like policy. Proportional, iterative, exportable. Relation to Seoul Commitments + METR framing. Voluntary commitment, not regulation.

1. Background — FAL model (import table from #5 §5, do not duplicate prose). ASL→FAL translation note. FAL-2 is MVP. FAL-4 undefined by design (RSP ASL-4 pattern).

2. Capability Thresholds and Required Safeguards — Per FAL: threshold (what agents can do with credits) + required safeguards (deployment + security + governance). Pause rule stated here. Re-evaluation commitment here. Include summary matrix from #5 §5 (4-row table).

3. Capability Assessment — How you know a threshold is crossed. Elicitation standard (match adversary effort). Cadence statement. Links to #7 harness doc.

4. Safeguards Assessment — How you know safeguards meet the standard. Per FAL: deployment checks + security checks + governance checks. Reference Implementation links (file:line + test file:line).

5. Follow-Up Capability Assessment — Drift check. What to run when code/params changed without bumping FAL.

6. Deployment and Scaling Outcomes — 6.1 Continue / 6.2 Restrict (pause). Concrete examples: FAL-2 violation → genesis reset + hot-patched release; FAL-3 proposal without safeguards → PR closed.

7. Governance and Transparency — Maintainer-as-RSO; external reviewer pool; PR gating; procedural review; anti-retaliation; rsp-updates page analogue; feedback channel.

Appendix A: Glossary — FAL, Capability Threshold, Required Safeguards, Dust Threshold, Fork-Choice, Whitewash Cost, Tenure-Weighted Decay …

Appendix B: FAL-2 Standard (MVP safeguards fully specified) — Copy #5 FAL-2 required safeguards block + link to impls.

Appendix C: Detailed Capability Thresholds — Import dossier from #5 (do not summarise — preserve nuance per eval reader).

Appendix D: Changelog — Version | Date | Change | Redline (`git diff safety/vX..safety/vY`)

Appendix E: External Reviewers — Pool + approval note.

Appendix F: Compliance Checklist — Procedural steps checked annually (see §6 template).
```

### 5.3 What #5 supplies vs what this ticket supplies

| Question | Owner |
|---|---|
| What are FAL-1..4 thresholds and required safeguards? | **#5** (`research/safety-risk-assessment` §5 + matrix). `SAFETY.md` imports, not restates. |
| What is the safety case for MVP at FAL-2? | **#5 §4 blast radius table** — cite as "because #5 shows blast radius is *nuisance + DoS within network*, fixed-supply caps it." |
| What are the per-FAL checks, in what order, with what evidence? | **#7** (evaluation framework). `SAFETY.md` references `#7`'s harness; does not embed test code. |
| How to *publish* the above so it's legible + auditable? | **This ticket (#6)** — document shape, governance downscaling, public-vs-internal split, signing/transparency-log mechanics. |

### 5.4 Minimal MVP `SAFETY.md` snippet (so maintainer can ship tomorrow)

Include in `SAFETY.md` verbatim for FAL-2 justification (word budget ~300 words):

```markdown
## FAL-2 Safety Justification (MVP)

**Claim:** The MVP at FAL-2 (fixed-supply virtual credits, accounts + transfers + reputation only, no escrow/lending/fiat bridge, Rust nodes on user hardware) presents no meaningful external financial risk. Worst case (§4 of research/safety-risk-assessment: blast radius table) is ledger corruption + reputation dilution + DoS within the network — recoverable by genesis reset. This satisfies the "contained" safety case for FAL-2.

**Why fixed-supply matters:** The single design choice that caps harm. Without external value, no real-world loss; without mint beyond genesis, no inflation; without leverage, no cascade. See research/safety-risk-assessment §4.2–4.3.

**Pause trigger to FAL-3:** Any proposal to add conditional transfers, lending/margin/interest, fiat/crypto bridge, or policy-based autonomous spending crosses the FAL-2→FAL-3 threshold. Per the pause rule (§6.2), such a proposal may not be merged until FAL-3 required safeguards (multi-party mint auth, SLSA provenance on ledger code, defense-in-depth classifiers, external red-team — §5 of that dossier) and corresponding evaluations (research/safety-evaluation-framework §… ) are implemented and show no meaningful catastrophic misuse under world-class adversarial testing.

**What we evaluated for FAL-2:** [links to risk-report-YYYY-MM-DD + `cargo test` names]. Coverage date / publish date / commit recorded in that report. Raw exploit payloads withheld per redaction rules (§3), marked [REDACTED].

Next evaluation: ≤6 months or before any FAL-relevant PR, whichever is sooner.
```

---

## 6. How to Make Commitments Auditable without Central Authority

Goal from ticket #6 text: *"How to make safety commitments that are auditable without a central authority (e.g. signed releases, transparency log)."*

AI Bank has no foundation server. Use **Git + Sigstore public Rekor** — both are *already* central but *not yours* to fund, and both are **content-addressed + mirrored**, so verification does not require liveness of your host.

### 6.1 Git as the primary transparency log (free, always-on)

Git already supplies an append-only log with hashes, authorship, and signed-tag provenance. Use it deliberately:

```bash
# Maintainer creates signing key once
gpg --full-generate-key   # or ssh key (Git 2.34+ supports ssh signing)
git config gpg.format ssh  # if using ssh key
git config user.signingkey ~/.ssh/ai-bank_ed25519.pub

# Every SAFETY.md-affecting release
git tag -s safety/v0.1.0 -m "SAFETY.md v0.1.0 effective 2026-09-02 (FAL-2 MVP)"
git push origin safety/v0.1.0

# Verifier (no trust in GitHub UI)
git fetch --tags
git verify-tag safety/v0.1.0
git show safety/v0.1.0:SAFETY.md
git log --follow -- SAFETY.md            # changelog is the log
git diff safety/v0.1.0..safety/v0.2.0    # redline is the diff
```

Conventions:
- Tag name `safety/v<semver>` distinct from code version `v<semver>` (safety policy evolves at different cadence from code).
- Signed tags use **ssh or gpg**; both verify with `git verify-tag`.
- Store maintainer public key in repo: `docs/safety/MAINTAINER_PUBKEYS.asc` + `/.github/trusted-keys/` so forks can verify without out-of-band key exchange.
- Enforce via GitHub protected tag pattern `safety/v*` requiring signed tags (repo settings — not code-level).
- The `rsp-updates` analogue is simply **`git log -- SAFETY.md` + GitHub Releases page + `docs/safety/updates.md` append-only journal** (link to tags). No hosted DB to fund.

### 6.2 Sigstore Rekor for release artifacts (no key management)

When you publish a node binary / crate, don't just push bytes — **sign provenance and log it**.

**At publish (GitHub Actions — `actions/checkout` + `sigstore/cosign` or `npm`-style `sigstore-js`/`cargo-sigstore`):**

1. Builder obtains **Fulcio short-lived cert** from OIDC (GitHub Actions ID token = proof of workflow identity).
2. Builds `node-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`, computes digest.
3. Generates **SLSA provenance** (in-toto statement with SLSA predicate: source URI, builder ID, commit SHA, build steps, materials) — see `slsa-github-generator`.
4. Signs provenance with ephemeral key + Fulcio cert → uploads entry to **public Rekor** `rekor.sigstore.dev`.
5. Bundle `{ signature, cert, Rekor inclusion proof (SCT + Signed Tree Head), provenance }` attached to **GitHub Release**.

**At verify (any node operator, no server of yours):**

```bash
# Rust verifier (generic SLSA)
slsa-verifier verify-artifact --provenance-path provenance.intoto.jsonl \
  --source-uri github.com/kudosscience/ai-bank \
  --builder-id https://github.com/slsa-framework/slsa-github-generator/.github/workflows/generator_generic_slsa3.yml \
  node-v0.1.0-x86_64-unknown-linux-gnu.tar.gz

# Or Rekor direct verify (cosign)
cosign verify-blob --bundle cosign.bundle --certificate cert.pem node.tar.gz
rekor-cli verify --rekor_server https://rekor.sigstore.dev --entry <entry>
```

What is checked (mirrors npm provenance verification steps from OSSF doc):
- Artifact's digest == subject in provenance.
- Cert issued by Sigstore trusted root (via TUF root) and **SCT** within cert validity window.
- Rekor received entry *while cert was valid* (non-repudiation).
- `Source Repository URI` / `Builder ID` / `commit SHA` match expected repo — **not attacker-forked**.
- Inclusion proof cryptographically valid against log's Signed Tree Head (witness-cosigned in Rekor v2).

**Operational notes specific to AI Bank:**
- Public Rekor entries are **public and permanent**. Never include raw exploit payloads, trace logs with IPs, or secrets in the predicate — only `digest + repo URI + commit SHA + redacted eval summary + "FAL-2 checks passed @ coverage 2026-09-02"`.
- Use **ephemeral keys** (destroyed after signing); trust is in Fulcio + Rekor, not in a long-lived maintainer key that can be stolen.
- Store bundle **alongside** the binary — not just in Rekor. Verifier can verify offline from bundle + TUF-fetched trust root, without contacting Rekor at verify time (recommended Rekor v2 flow).
- Set up **rekor-monitor** workflow to watch your identity (`fulcio: github.com/kudosscience/ai-bank`) for unauthorised entries — detects OIDC compromise.

### 6.3 What to sign per release — minimum set

| Artefact | What it proves | How to verify |
|---|---|---|
| `SAFETY.md` (tag `safety/vX`) | Policy version you claim to satisfy | `git verify-tag safety/vX` |
| `node` binary / `ai-bank` crate tgz | That bytes were built from `commit SHA` in trusted builder | `slsa-verifier` / `cosign verify` + `rekor verify` |
| `risk-report-YYYY-MM-DD.md` + `provenance.intoto.jsonl` | That eval summary was attested on `coverage_date` and logged in Rekor | `slsa-verifier` on report digest + `rekor verify` |
| `CHANGELOG.md` + `SAFETY.md#changelog` | That no silent policy change occurred | `git diff safety/vA..safety/vB` + Rekor trace |
| `SECURITY.md` / `MAINTAINER_PUBKEYS.asc` | That disclosure path is authentic | `gpg`/`ssh` verify |

### 6.4 `cargo audit` / `cargo deny` as Security Standard hygiene

For FAL-2's supply-chain controls (shrunken from RSP's 17), run:

```bash
cargo audit                 # known vulns
cargo deny check advisories bans licenses sources
```

Add **SLSA `cargo publish` provenance** when stable (>SLSA Build Level 1); until then, GitHub Release bundle is the attestation. Record supply-chain posture in `SAFETY.md` Appendix B line: "Supply-chain: `cargo audit` clean @ <commit>, reproducible build via `cargo build --locked`."

---

## 7. Update Cadence & Transparency Mechanisms

### 7.1 Concrete cadence for AI Bank (downscales RSP)

| Cadence | What fires | Artefact |
|---|---|---|
| **Per release that touches `crates/*` or transfer/reputation logic** | Run FAL-2 harness (from #7) → cut Risk Report note | `docs/safety/risk-report-YYYY-MM-DD.md` |
| **At least every 6 months** even if no release | Re-evaluate thresholds vs new knowledge; publish `updates.md` entry with `coverage_date` | `docs/safety/updates.md` (heartbeat) — mirrors `rsp-updates` |
| **When Required Safeguards upgrade** (e.g., add dust filter, change fork-choice) | Re-evaluate Capability Thresholds per v2.1 commitment | `SAFETY.md` Changelog + issue thread |
| **On pause trigger** | Within 7 days, publish incident note with pause reason + remediation plan | GitHub issue `safety` + `pause/YYYY-MM-DD.md` |
| **Annually** | Procedural compliance review (did we follow §3–§7?) | `docs/safety/procedural-review-YYYY.md` |

Rule: **depth over frequency** — RSP v2.0 lengthened 3→6 months for this reason. Do not promise per-week evals you can't staff.

### 7.2 Redlines & version history (the `rsp-updates` pattern)

- Every `SAFETY.md` change ships with a **redline**: `https://github.com/kudosscience/ai-bank/compare/safety/v0.1.0...safety/v0.2.0` (GitHub compare page). Link from Changelog — exactly how RSP distributes `redline` PDFs alongside each version.
- Keep all historical versions reachable: tags + `git show` + pinned PDFs at `docs/safety/SAFETY-v0.1.0.pdf` if you want immutable rendered form.
- Include in Changelog: `| 0.2.0 | 2026-12-01 | Tightened FAL-2 dust threshold 1→10 credits (replayed dust flood) | [compare](link) |`

### 7.3 Public feedback channel (exportable & minimal)

Include in `SAFETY.md` §7 footer, mirrored from RSP's `rsp@anthropic.com`:

```markdown
Feedback on this safety policy: open a GitHub Issue with label `safety:feedback`
or email <maintainer>. We distinguish editorial fixes (typos, clarifications)
from threshold changes — the latter follow §4.2 gating.
```

BIP-style supporting infrastructure: **Discussions** or `bitcoindev` analogue is GitHub Discussions; monthly digest is free.

---

## 8. Recommendation for MVP — What to Ship Day One (Minimal but Honest)

To satisfy map #1's "Safety is part of the MVP" without labour-spiking:

1. **`SAFETY.md` v0.1.0** at repo root (use skeleton from §5.2, filled with import from #5 §5). Cost: one file + public review comment.
2. **`docs/safety/updates.md`** with first entry (MVP risk note): coverage date = genesis commit date, includes branch + commit + FAL-2 justification snippet (§5.4) + evaluation summary ("harness §… passed, dust flood capped, Sybil spot-check within tolerance, known gaps: … [REDACTED]").
3. **Signed tag `safety/v0.1.0`** pushing `SAFETY.md` + first Risk Report note.
4. **Release bundle signed via Sigstore** (GitHub Release + Rekor entry) for `ai-bank` node binary.
5. **Procedural checklist appendix** (`Appendix F`) listing which RSP steps you actually ran for this release (so a reviewer can see what was skipped intentionally at FAL-2 vs accidentally).

Do not ship a "comprehensive Risk Report" wrapper at FAL-2 — that belongs to FAL-3 (§1.2's v3.0 machinery). Ship the *commitment* and the *heartbeat*; prove they will be kept.

---

## 9. Implications for Downstream Tickets

- **#7 Safety evaluation framework:** This doc defines *where* evaluation detail goes (public redacted vs internal) and *how* it is attested. #7 defines *what* to evaluate and *how well*. The two documents meet at `SAFETY.md` §3/§4 prose (written by #6) pointing to `#7`'s harness as the "implementation." Keep the seam clean: no test code in `SAFETY.md`; no threshold definitions in `#7`.
- **Ledger / reputation design:** Appendix B of `SAFETY.md` references these tickets. If they alter safeguards (e.g., fork-choice, reputation decay half-life), they must update `SAFETY.md` Changelog (otherwise commitments and code diverge).
- **Future `docs/safety/` ownership:** After this ticket, `docs/safety/updates.md` and `docs/safety/risk-report-*` are routine files — not research artefacts. Graduate them to primary docs once `SAFETY.md` lands on `main`.

---

## 10. Open Questions / Deliberate Non-Decisions

1. **Exact `SAFETY.md` word count and renderer?** Prefer Markdown (like `SECURITY.md`) at MVP; consider a pinned PDF only for tag archival. Do not gate MVP on deciding PDF tooling.
2. **Who is the external reviewer?** Not chosen here — leave pool empty at `v0.1.0` and record ".. to be enumerated" in Appendix E, with intent to fill before any FAL-3 proposal. This mirrors RSP's pre-review "to be selected" placeholders.
3. **Is noncompliance reporting standalone?** At FAL-2, fold it into `SAFETY.md` §7; spin a separate `NONCOMPLIANCE.md` only if reporter flow justifies it (RSP did so at Feb 2026, after a year of operating history — that timing is the lesson: don't over-process early).
4. **Rekor vs local-only attestation?** Rekor is public — if project wants a fully offline trust path, add SSH-signed `attestations/<coverage>.asc` alongside Rekor. The simplest audit path is "both": verify offline via signed tag *and* online via Rekor.
5. **FAL-4 is still "undefined by design"** — do not pin it in `SAFETY.md` beyond the one-line placeholder from #5. Like RSP ASL-4, defining it too early is over-rigid.

---

## Appendix A: Primary Sources

- Anthropic — `anthropic.com/responsible-scaling-policy` hub + PDFs: v1.0 (Sep 19 2023), v2.0 (Oct 15 2024), v2.1 (Mar 31 2025), v2.2 (May 14 2025), v3.0 (Feb 24 2026), v3.1 (Apr 2 2026), v3.2 (Apr 29 2026), v3.3 (May 26 2026), v3.4 (Jul 8 2026) — plus `rsp-updates` / `rsp-updates` changelog pages and `RSP Noncompliance Reporting and Anti-Retaliation Policy` (Feb 2026, expanded Mar 24 2026). Full hub fetched 2026-09-02 via WebFetch (markdown render).
- Anthropic RSP PDFs v2.0–v3.4 sourced via `websearch` excerpts (case: `anthropic.com/responsible-scaling-policy`, `cdn.sanity.io` PDFs). TOC + section numbering + capability-threshold appendix + changelog language verified from `616dee633636e5bd309cb73aed8622e80fe47839.pdf` (Oct 15 2024) and `responsible-scaling-policy-v3` announcement.
- Seoul Frontier AI Safety Commitments — `gov.uk/government/publications/frontier-ai-safety-commitments-ai-seoul-summit-2024` (May 21 2024). Commitments I–VIII, company list 16→20. Commitments VII (transparency) + VIII (external involvement) verbatim via `websearch`/`webfetch` (this doc cites the guide's gloss at `safetyframeworkguide.com`).
- Seoul *Guide to Writing Frontier AI Safety Frameworks* — `safetyframeworkguide.com` (step-by-step guide referencing OpenAI PF + Anthropic RSP + DeepMind FSF as templates; commitment VII tranching: public-safe vs trusted-actor detail).
- METR — `metr.org/common-elements` — "Common Elements of Frontier AI Safety Policies" (Aug 2024, updated Dec 16 2025). Quantifies presence of capability thresholds (9/12), weight security (11/12), deployment mitigations (12/12), halt-conditions, accountability, update policy.
- OpenAI Preparedness Framework (beta, Dec 2023) + Google DeepMind Frontier Safety Framework (May 2024) — cited via Seoul guide + FMF synthesis; referenced for scorecard / CCL pattern.
- Frontier Model Forum — "Introducing the FMF's Technical Report Series" (Apr 22 2025) — lists Risk Taxonomy / Capability Assessments / Mitigations & Safeguards / Third-Party Assessments / Risk Governance as report series axes.
- Ethereum Foundation — Mandate (Mar 13 2026) — `ethereum.org/foundation/mandate` + canonical PDF `ef-mandate.pdf` + blog `blog.ethereum.org/2026/03/13/ef-mandate`. Patterns: governance minimisation, public & auditable, forkable, "Only-EF rule", compounding upstream leverage, walkaway test, CROPS (censorship-resistant, open-source, private, secure), zero-option.
- Bitcoin BIPs — `bitcoin/bips` repository + specs: `bip-0001.mediawiki` (field definition, process categories), `bip-0002.mediawiki` / `bip-0003.md` (Updated BIP Process — Status `Draft → Complete → Deployed`, Mandatory Changelog, reference implementation + test vectors before `Complete`, BIP Editors as librarians). + `bip-0132` / `bip-0002` committee acceptance prototype. Sourced via `websearch` on `bitcoin.org/bip/2` + GitHub `bitcoin/bips` raw.
- Sigstore / Rekor — `docs.sigstore.dev/about/security`, `docs.sigstore.dev/logging/overview`, `github.com/sigstore/rekor` README, `sigstore/architecture-docs/rekor-v2-spec.md` (tiles, witnesses, checkpoint cosigs, offline-verification bundle), `rekor.sigstore.dev` public instance.
- SLSA + npm provenance — `github.com/npm/provenance`, `github.com/ossf/wg-securing-software-repos/docs/build-provenance-for-all-package-registries.md` (OIDC → Fulcio → Rekor → registry publish attestation flow), `slsa.dev/blog/2023/05/bringing-improved-supply-chain-security-to-the-nodejs-ecosystem`, `github.com/slsa-framework/slsa-verifier` (verify-artifact / verify-npm-package with `--source-uri`/`--builder-id`).

Style note for verifiability: every section that defines a *requirement* cites the source that owns it (RSP PDF/page, Seoul guide text, EF Mandate sentence, BIP spec, Sigstore architecture doc). Where this doc synthesizes (e.g., `SAFETY.md` skeleton + governance translation), it marks synthesis as proposal, not quoted source — see `SAFETY.md` labelling recommendation ("Proposed structure" header).

---

## Appendix B: Style Note for Verifiability (reused from #5)

Keep `SAFETY.md`'s commitments independently verifiable:

- Every threshold row cites the dossier that defines it (link to `#5` branch commit or `Appendix C` after import).
- Every required safeguard cites the commit+line that implements it and the test that exercises it (`cargo test — <test_name>` is executable; link to file:line).
- Every evaluation summary cites branch + commit + coverage date + builder ID; attestation digest resolves to Rekor entry.
- Where synthesis invents (e.g., permission vs threshold phrasing), label it `Proposal` so reviewers know not to chase a source.

This is the "follow every claim back to the source that owns it" discipline from `/research` skill spec.

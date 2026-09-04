# ADR 0012: Project is ClawBank (display) / clawbank (code, slug, wire)

The project formerly called AI Bank is ClawBank. Display form `ClawBank`; code, slug, filesystem, and wire form `clawbank` (Rust/binary convention). The rename is total for everything not yet shipped: wherever ADRs 0001-0011 say `ai-bank` in an identifier - signing domains, gossipsub topics, registry keys, crate and binary names, home-directory paths, artifact names - read `clawbank`. ADRs 0001-0011, research docs, and existing issues stay byte-identical as history.

## Status

Accepted — grilling, all recommendations accepted. Supersedes the naming portions of ADRs 0001-0011 only; their technical decisions stand unchanged.

## Context

OpenClaw-inspired naming for a bank for LLM agents. No code, network, or value exists yet, so a total rename costs one commit and a slug redirect; after shipping, the same rename would fork the wire protocol and strand the slug. The name must not imply affiliation: agent scope stays general LLM agents per the map Destination.

## Considered Options

- **Total rename now, history frozen (chosen)** - Casing locked: `ClawBank` in prose, `clawbank` in slugs, crates (`clawbank-types`), binary/CLI, `~/.clawbank/`, domains (`b"/clawbank/1/"`), topics (`/clawbank/transfer/1.0.0`, `/clawbank/evidence/1.0.0`, `/clawbank/checkpoint/1.0.0`), keys (`/clawbank/peer/`, `/clawbank/registry/`, `/clawbank/snapshot/`), artifacts (`clawbank-docs-*`). GitHub slug becomes `kudosscience/clawbank` with redirects preserving every existing link. ADRs, research branches, and issues keep `ai-bank` as the record of what was decided under that name.
- **Human-facing only, wire frozen (rejected)** - split-brain naming with zero deployed compatibility to protect; taxes every future reader forever.
- **Slug/display only, code later (rejected)** - defers the cheap part of the rename to the expensive moment (post-ship); guarantees a second rename.

## Consequences

- Name inspiration only: no affiliation with OpenClaw, no narrowing of agent scope; recorded here so the name cannot be read as endorsement.
- The freed `ai-bank` slug could be claimed by a third party, breaking old-URL redirects; accepted residual risk, mitigated by renaming before the project is public-facing.
- Build tickets (#20+) need no edits: they reference ADR files (paths unchanged) and track names (unchanged); implementers apply this ADR's mapping rule when writing code.
- Local clone directory rename is out of scope for this change (machine-local, invisible to the project).
- Link maintenance exception to the freeze above: absolute URLs pointing at the old slug may be updated in place across frozen docs (same target, new address) without violating it. Rationale: GitHub redirects renamed-repo subpaths inconsistently (repo root redirects; per-issue pages 404), so stale issue links rot for readers while link-check CI fails. Added 2026-09-04 after the first red main build post-rename.

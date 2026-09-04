# ADR 0011: Genesis mints 1B credits (1e15 base units) via signed file-based artifact, with pinned-hash boot and three-tier social-fork recovery

Network genesis is a single mint event producing `SUPPLY = 1_000_000_000_000_000` base units (1 billion credits, 6 decimals, 1 credit = 1,000,000 units) allocated by file-based `BTreeMap<PeerId,u64>` as an equal split among the bootstrap PeerId set, committed in a maintainer-signed `genesis.json` artifact. Nodes refuse to start on hash-pin mismatch and log the pin every boot. New nodes load → verify → seq-0 insert → DHT → gossipsub → sync-from-0. Recovery is pause tag → checkpoint pin → new genesis if distribution is rejected — see decision record.

## Status

Accepted — implements wayfinder ticket [#12 Genesis & credit minting: supply, allocation, and bootstrap](https://github.com/kudosscience/clawbank/issues/12) (grilling, all recommendations accepted). Depends on ADRs 0001/0002/0004/0005/0006/0007/0008/0009/0010.

## Context

FAL-2 is genesis-mint-only with a contained blast radius (ADR 0004): allocation fairness needs no Sybil-proof mechanism because recovery is a social fork, not a bailout. Amounts are `u64` with `u128` sum accumulator and `DUST=100`/`MAX=SUPPLY` pinned in genesis (ADR 0007); the artifact shape `{supply, balances, checkpoint_seq: 0, history_hash: blake3, sig}` over `b"/ai-bank/1/genesis:"` is fixed by ADR 0009. Left open: the supply number, the allocation rule, boot strictness, join order, and the recovery runbook.

## Considered Options

- **1e15 base units, file-based equal split, strict boot, ordered join, three-tier recovery (chosen)** — `SUPPLY=1e15` fits `u64` with 4 orders headroom; `DUST=100` (= 0.0001 credit) keeps micro-pricing viable (a 1e6 total supply would not). Allocation is a committed file, not a claim window: proof-of-key claim rejected (day-0 Sybil with free keypairs captures distribution, research harm #1); invite codes deferred to FAL-3. Boot rule: bundled genesis hash must equal release-tag pin or node refuses to start, and the pin is logged every boot so divergent geneses fork loudly. Join order: (1) load genesis (bundled, or release HTTPS + verify sig + pin + `sum==SUPPLY`) → seq-0 insert; (2) dial bootstrap, `identify → kad.add_address` + `mdns` (ADR 0008); (3) subscribe `/ai-bank/transfer/1.0.0` with `validate_messages=true`; (4) `SyncRequest{since_seq: 0}` catch-up (ADR 0009). DHT before gossip so sync has peers; never trust DHT-served genesis without signature. Recovery: (i) `git tag -s pause/YYYY-MM-DD` + `safety` issue immediately (ADR 0006); (ii) `Checkpoint{seq,hash,signers}` pinned and published (`/ai-bank/checkpoint/1.0.0` + tag + Rekor) if history salvageable (ADR 0009); (iii) new `genesis.json` with revised map + version bump + Changelog + Risk Report if distribution rejected, community re-joins (ADR 0004/0005).
- **Small supply / claim window / lenient boot (rejected)** — breaks dust economics, Sybil-drains distribution, or forks silently at seq 0.

## Consequences

- `SUPPLY`, `DUST_THRESHOLD`, `MAX_AMOUNT`, half-life, and minima all live in `genesis.json`/`META` (ADRs 0007/0010); changing any is a soft fork → checkpoint + `SAFETY.md` Changelog + harness re-run (ADRs 0005/0006).
- `genesis.json` ships with the binary; release pipeline attests it via Sigstore/Rekor bundle (ADR 0005 §6) alongside the node binary.
- Glossary fixed in `CONTEXT.md`: genesis (event) vs genesis artifact (file), credit vs base unit, checkpoint, social fork.

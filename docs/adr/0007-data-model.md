# ADR 0007: AccountId is PeerId, Transfer/SignedBatch are the canonical types with CBOR/JSON dual-codec and redb persistence

The canonical data model is `AccountId = PeerId` (Ed25519, ADR 0001), `Transfer { from, to, amount: u64, nonce: u64, timestamp: u64, sig: [u8;64] }` and `SignedBatch { version:1, author: PeerId, seq, transfers, vclock, sig }` over domain `b"/ai-bank/1/transfer:"` / `b"/ai-bank/1/batch:"`, derived once with `serde`+`schemars` and serialized as CBOR (gossipsub/request-response/redb/signing) or JSON (axum + rmcp) via `is_human_readable`. Amounts are `u64` with `DUST_THRESHOLD=100`/`MAX_AMOUNT=SUPPLY` pinned in genesis/`META`, sum==SUPPLY enforced with `u128` accumulator. Storage is `redb 4.x` tables `balances`/`nonces`/`batches`/`meta` with one atomic `write_txn` per batch — see `research/data-model` decision record.

## Status

Accepted — implements wayfinder ticket [#10 Account & transaction data model](https://github.com/kudosscience/clawbank/issues/10) → `research/data-model` (`docs/research/data-model.md:1` on `research/data-model`). Depends on ADRs 0001/0002/0003/0004, unblocks ledger replication and reputation/genesis.

## Context

After fixing identity (PeerId), transport (libp2p Noise/TLS+Yamux+QUIC + identify/kad), and API plane (localhost axum + rmcp on `127.0.0.1`), the shared type system must decide account identity, wire encoding, amount limits, and persistence versioning before ledger fork-choice and reputation can be implemented without a cloud bill.

## Considered Options

- **AccountId = PeerId (chosen)** — `type AccountId = PeerId`; raw `PeerId::to_bytes()` multihash as `redb` key, base58 `12D3Koo…` only in JSON. Same Ed25519 key for handshake and transfer sigs; no derivation. Deferred: HD `blake3(PeerId||index)` and `Delegation{account→delegate, expiry, seq, sig}` (UCAN/macaroon/biscuit pattern) for FAL-3 as additive tables/topics.
- **Flat Transfer + SignedBatch (chosen)** — per-transfer `sig` over canonical CBOR plus optional per-batch `sig` over `DOMAIN_BATCH`; `nonce` is monotonic `seq` (1-indexed, 0 reserved for genesis), `timestamp` ms since epoch informational with 5 min skew, ordering by `nonce`/`seq` per fork-choice. `deny_unknown_fields`, frozen field order = canonical CBOR.
- **Dual-codec one type (chosen)** — single `#[derive(Serialize,Deserialize,JsonSchema)]` in `ai-bank-types`; `peer_id_serde` branches on `is_human_readable()` (base58/hex in JSON, bytes in CBOR); `cbor4ii 0.3` for P2P/storage/signing, JSON for `axum::Json<T>`+`rmcp #[tool]`, `schemars 1.1` as single schema reused for `utoipa` OpenAPI and `rmcp` `inputSchema`. Alternative `schemars`→OpenAPI per-transport DTO duplication rejected.
- **Amounts u64 + u128 accumulator (chosen)** — 6 decimals → `SUPPLY=1e15` fits `u64`, `total: u64` fast-path + periodic `u128` full-scan for `sum==SUPPLY`; `DUST_THRESHOLD`/`MAX_AMOUNT` consensus-pinned in `genesis.json`/`META` so `validate_messages` deterministic; `u128` per-account rejected (wasted), string amounts rejected.
- **redb 4.x atomic batch (chosen)** — tables `BALANCES: Table<&[u8],u64>` / `NONCES` / `BATCHES` / `META`, one `write_txn` per batch atomic (`begin_read` concurrent for API/reputation), `meta.schema_version: u32=1` + `SignedBatch.version: u8=1` + topic `1.0.0` versioning, additive-only serde evolution with `Checkpoint` cutover on reorder; `sled 0.34.7` (beta, unstable format) rejected; transport-blind `service::validate/apply/balance` shared by `api` + `p2p`.
- **API alignment** — `axum 0.8` + `rmcp 3.2` handlers call same service with same `JsonSchema`; crate pins `libp2p 0.56 + identity 0.2 + cbor4ii 0.3 + schemars 1.1 + axum 0.8 + rmcp 3.2 + redb 3.1/4.x` (MSRV 1.83).

## Consequences

- Ledger (`#9`) imports `Transfer`/`SignedBatch` verbatim for gossipsub `IdentTopic("/ai-bank/transfer/1.0.0")`/req-res `cbor`/`redb`/`signing_bytes` (RFC 8949 deterministic, ~40% smaller than JSON); genesis (`#12`) supplies `SUPPLY`+`BTreeMap<PeerId,u64>` + `blake3` pin; reputation (`#11`) reads `redb` via `begin_read` without coupling fork-choice.
- Signing bytes are canonical CBOR framed by domain; verification is `account_vk.verify(domain+cbor, sig)` + `PeerId::is_public_key`; `ValidationMode::Strict` drops unsigned/invalid at mesh.
- Versioning: DB `META.schema_version=1` and value `version=1` with `bytes.push(version)` prefix; bumping either is breaking → `Checkpoint` migration; `cbor4ii` vs `ciborium` still TBC in doc §11 (non-blocking).
- Open questions deferred to `research/data-model` §11 (supply denomination, dust consensus vs local, first-nonce 1 vs 0, `BATCHES` key shape, `cbor4ii` vs `ciborium`) do not block implementation start.

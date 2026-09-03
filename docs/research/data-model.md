# Account & Transaction Data Model: Canonical Structures for Identity, API, Transport, Ledger, Persistence

**Wayfinder Research Ticket #10 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**
**Branch:** `research/data-model` | **Date:** 2026-09-03 | **Author:** Muse Spark (research subagent)
**Status:** Research complete — decision-ready
**Depends on:** ADR 0001 (Ed25519 `PeerId`), ADR 0002 (libp2p swarm: `gossipsub` + `request-response` CBOR), ADR 0003 (localhost `axum` + `schemars` shared with `rmcp`), ADR 0004 (FAL-2 fixed-supply), ledger-replication (#9)
**Blocks:** #9 (ledger replication pulls record shape from here), #11 (reputation keys off `AccountId`), #12 (genesis allocation table), persistence/transport wiring

---

## TL;DR for Decision-Maker

| Question | Option | Verdict for AI Bank MVP |
|---|---|---|
| **A: Account ID** — `PeerId` directly vs derived key vs delegation `node_key → account` | `PeerId` string `12D3Koo…` is the `AccountId`; no derivation, no delegation cert for MVP | **Recommended: `type AccountId = PeerId`.** One keypair per account, zero indirection, identical key for Noise/TLS handshake and for transfer sigs. Derivation/delegation deferred to FAL-3 as separate feature (see §1). |
| **B: Transfer schema** — `{from,to,amount,nonce,sig,timestamp}` | Flat `Transfer { from: PeerId, to: PeerId, amount: u64, nonce: u64, timestamp: u64, sig: [u8;64] }` + batch wrapper `SignedBatch { transfers, sig }` with domain `b"/ai-bank/1/"` | **Recommended: flat struct with per-transfer sig + optional per-batch sig.** `amount: u64`, `nonce: u64` strict monotonic, `timestamp: u64` ms since epoch (non-consensus, informational), `sig: 64B Ed25519` over canonical CBOR. See §2. |
| **C: Encoding** — CBOR vs JSON | **CBOR (canonical, deterministic) for wire (`gossipsub` + `request-response`) and `redb` value bytes; JSON for HTTP/`rmcp` with same structs via `serde`** | **Recommended: dual-codec, one type.** Derive `Serialize/Deserialize + JsonSchema` once; serialize as CBOR on P2P/storage, as JSON over HTTP/MCP. Pick one CBOR crate (`cbor4ii 0.3` or `ciborium 0.2`) with deterministic map-key ordering. See §3. |
| **D: Amounts & limits** — `u64` vs `u128`, dust vs max | `u64` smallest unit (e.g. 6 decimals → `SUPPLY ≤ 10^15` fits `u64`), `u128` only for `sum` accumulator; `DUST_THRESHOLD` + `MAX_AMOUNT` as `u64` constants pinned in genesis/config, enforced at `validate_messages` gate | **Recommended: `u64` per-account, `u128` accumulator for `sum==SUPPLY` check.** Dust = 100 units, max = `SUPPLY` (or tight cap), both consensus-pinned (see §4). |
| **E: Balance + nonce storage** | `redb` typed tables `balances: Table<PeerIdBytes, u64>`, `nonces: Table<PeerIdBytes, u64>`, `transfers: Table<u64|Composite, CBOR>` with one `write_txn` per batch covering both, `sum(balances)==SUPPLY` invariant checked on copy before commit | **Recommended: `redb 3.1/4.x` with two tables + `meta` + `transfers` log.** O(1) fast-path `total: u64` field + periodic full-scan audit in `u128`. See §5. |
| **F: API alignment** — `axum::Json<T>` + `schemars` shared with `rmcp` | Single `ai-bank-types` crate with `#[derive(Serialize, Deserialize, JsonSchema)]` on every API/P2P type; `axum` handlers use `Json<T>`, `utoipa`/`aide` generates OpenAPI from same `JsonSchema`, `rmcp #[tool]` reuses same types for `inputSchema`/`outputSchema` | **Recommended: shared-types crate, one source of truth.** Structs annotated once, no per-transport DTO duplication. See §6. |
| **G: Persistence versioning** | `meta` table holds `schema_version: u32`, `genesis_hash: [u8;32]`, migration via `write_txn` copy-on-write; CBOR values carry `version: u8` prefix or use `serde` `default` + `deny_unknown_fields` discipline; file-format stability via `redb` stable format since 1.0 | **Recommended: explicit `schema_version` + CBOR `version` byte + additive-only `serde` evolution.** Documented migration path forward (see §7). |

**Bottom line:** Ship MVP on **`PeerId = AccountId` + flat `Transfer` with `u64` amount/nonce/timestamp + `Ed25519` domain-separated sig + CBOR for P2P/storage and JSON for HTTP/MCP from the same `Serialize+JsonSchema` derives + `redb` tables `balances/nonces/transfers/meta` with atomic batch commit + `sum==SUPPLY` invariant + explicit `schema_version`. No delegation, no derived keys, no separate DTOs for MVP. Everything else is FAL-3 extension.

---

## 1. Account Identity: PeerId vs Derived Key vs Delegation

### 1.1 What `PeerId` Is (Binding Identity to Ledger)

Per ADR 0001 / `research/node-identity`, each node generates an `Ed25519` keypair once (`Keypair::generate_ed25519()`) and derives a `PeerId` as `multihash(protobuf(public_key))` encoded `12D3Koo…` (base58btc) or `bafz…` (base32 CIDv1) [Source: `specs/peer-ids/peer-ids.md` — identity multihash for ≤42B Ed25519 keys, `PeerId { multihash }`][Source: `libp2p/rust-libp2p/core/src/peer_id.rs` — `from_public_key`, `is_public_key`][Source: `libp2p-identity` — `Keypair`, `to_protobuf_encoding`/`from_protobuf_encoding`].

The same private key that authenticates the Noise XX / TLS 1.3 handshake (libp2p links ephemeral X25519 to long-term Ed25519 via `identity_sig` over `b"/ai-bank/1/"` domain per ADR 0002) is therefore available to sign ledger transfers. Using the identical key for both avoids a second key-distribution problem.

### 1.2 Comparison Table

| Dimension | **A: `PeerId` directly as `AccountId` (recommended MVP)** | **B: Derived account key** (`AccountId = blake3(PeerId \|\| index)` or BIP-32 child) | **C: Delegation `node_key → account` cert** (`Delegation{ account: PeerId, delegate: PeerId, seq, sig_account, expiry }`) |
|---|---|---|---|
| **What it is** | `type AccountId = PeerId;` — one node = one account, or one node = N accounts by generating N keypairs (each is its own `PeerId`). Ledger `balances` keyed by `PeerId` bytes directly. | One long-term master seed derives many `AccountId`s via `blake3` or `ed25519-bip32` hardened path `m/…'/index'`. Each `AccountId` has its own `VerifyingKey` distinct from the node's transport `PeerId`. | Node's transport `PeerId` acts as agent; a separate `AccountId` (maybe also a `PeerId`) signs `Delegation{ delegate: node PeerId, permissions: TransferOnly, expiry, nonce, sig_account }`. Transfers are signed by `node_key`, verified by checking `delegation_chain: verify(account_sig over delegate) && verify(delegate sig over transfer)`. |
| **Key management** | Single file `~/.ai-bank/identity.key` (`to_protobuf_encoding`) per account [Source: ADR 0001 consequences — file `~/.ai-bank/identity.key`]. N accounts = N files or `accounts.json` with encrypted blobs. | Needs HD derivation lib (`ed25519-dalek-bip32`, `slip10`) + secure master seed storage; adds spec surface (path, hardened flag). | Needs two keypairs + cert storage (`delegations` table) + revocation/expiry tracking. |
| **Sig verification in `validate_batch`** | `verify(batch.from == recovered PeerId, sig over domain || canon(batch))` — one `ed25519_dalek::VerifyingKey::verify` per transfer, `PeerId::from_public_key` check [Source: `libp2p_identity::PublicKey::verify`, `PeerId::from_public_key`]. | Must map `AccountId → VerifyingKey` table; transport `PeerId` ≠ ledger `AccountId` — gossipsub `PeerId` is unauthenticated for ledger authorship. | Two verifications per transfer + `Delegation` lookup + expiry/seq check; `PeerId` (gossip envelope) ≠ `from` (ledger). |
| **Rotation / loss** | Lose file → lose account (ADR 0001 operator responsibility, `export`/`import`). Rotation = new `PeerId` + signed `old→new` rotation statement (see ADR 0001 §Rotation). | Master seed loss = loss of all child accounts; child rotation via new index, but master still required. | Account key loss is fatal even if delegate survives; revocation via new delegation with higher `seq` + gossip of revocation cert. |
| **Petname / display** | Display `alias (short PeerId)` per ADR 0001 petname overlay; no ledger change. | Same petname layer, but now two namespaces (node alias vs account alias). | Petname applies to `account` and to `delegate` separately — UI must show both. |
| **Spec complexity** | Minimal; bounded by existing #9 ledger validation steps (sig → nonce → dust → balances → sum). | Medium; must specify derivation function, path, and pubkey recovery for light clients. | High; capability model (UCAN/Macaroons) — needs permission grammar, expiry, revocation propagation, replay of delegation `seq`. |
| **When justified** | Always for MVP — fixed-supply ledger does not need account indirection. | FAL-3+ when one operator runs many isolated balances (exchange, per-task escrow) or needs hot/cold key split. | FAL-3+ when AI agent process ≠ node operator (LLM delegates to node) with least-privilege scoping; see UCAN §1.4. |
| **Crate surface** | `libp2p-identity 0.2` alone. | `libp2p-identity` + `ed25519-dalek-bip32` or `tiny-bip39` + `blake3`. | `libp2p-identity` + bespoke `Delegation` struct (`Serialize+JsonSchema`), `redb` delegation table. |

**Recommendation for MVP: Option A.** Use `PeerId` byte representation directly as `AccountId`. No derivation, no delegation cert. The `balances` and `nonces` tables key on `PeerId::to_bytes()` (the multihash bytes, or the raw `VerifyingKey` bytes extracted via `PublicKey::try_from_protobuf_encoding` → `to_bytes`). Transport identity and ledger identity are the same keypair, so gossipsub's `MessageAuthenticity::Signed` key and the transfer `from` field match without extra lookup — this is the handoff contract `ledger-replication.md §6` deferred to #10 and confirmed here.

**Deferred designs (documented for FAL-3, not shipped):**

*Derived keys* — if later FAL-3 needs many accounts per node without storing N independent keypairs, add `AccountId = Blake3(PeerId_bytes || be32(index))` where `index` is explicit in the account address (like Bitcoin `m/44'/…`). Keep transport `PeerId` separate from ledger `AccountId`; verify transfers against `AccountId`'s `VerifyingKey` looked up in an `account_keys: Table<AccountIdBytes, VerifyingKeyBytes>` seeded at genesis or via `RegisterAccount{ account, vk, sig_account }` gossip. Do **not** use `secp256k1` or `RSA` — scope stays Ed25519-only per ADR 0001.

*Delegation* — for agent-to-nodeleast privilege (LLM agent process holds short-lived delegate key, node holds account key), define `Delegation { account: PeerId, delegate: PeerId, permissions: DelegationPerms, expiry: u64 ms, seq: u64, sig: [u8;64] }` where `sig = sign(account_sk, b"/ai-bank/1/delegation:" || cbor(DelegationWithoutSig))`. Verification: `account_vk.verify(delegation_bytes, sig) && delegate_vk.verify(transfer_bytes, transfer_sig) && now < expiry && delegation.seq > stored_seq[account][delegate]`. Store in `delegations: Table<(AccountId,DelegateId), DelegationCBOR>`. Revocation = new `Delegation` with `permissions: Revoked` and `seq = old+1`, gossipped on `/ai-bank/delegation/1.0.0`. Canonical prior art: [UCAN `ucan.xyz` — capability certificates `issuer → audience` with `attenuation`/`proof` chain](https://ucan.xyz) and [macaroons — caveat attenuation](https://research.google/pubs/pub41892/) — same pattern: account attenuates to delegate with caveats (amount cap, expiry). All of this is FAL-3+ (#11/#12 grilling territory); MVP keeps the attack surface flat by omitting it.

### 1.3 Why `PeerId` Bytes Are the Key Encoding (Not Strings)

* `PeerId::to_bytes()` = multihash bytes (identity-wrapped `protobuf(pubkey)` for Ed25519, ~36B). Stable, compact, valid as `redb` key (`&[u8]`).
* `PeerId::to_base58()` (`12D3Koo…`) is the human/JSON encoding. For CBOR ledger values and `redb` keys, use bytes; for HTTP JSON and logs, use base58 string via custom `serde` helper (see §2.1). This matches `libp2p` wire practice where `PeerId` is transmitted as bytes in protobuf, rendered as base58 only for display [Source: `PeerId::to_bytes` / `from_bytes`, `to_base58` / `from_str` in `peer_id.rs`].
* Never store `PeerId` as bare `VerifyingKey` bytes alone for the primary key — keep the multihash so `PeerId::is_public_key` checks remain valid without recomputing.

### 1.4 Delegation Prior Art (Why It Can Wait)

Existing capability-delegation schemes that justify deferral rather than invention:

* **UCAN (UCAN 0.10 / IPLD)** — JWT-like `header.payload.sig` where `payload = { iss: DID:key, aud: DID:key, cap, prf, exp }`, chained proofs `prf: UCAN`. Ed25519 DID `did:key:z6Mk…` is multibase-encoded `PeerId`-compatible. Relevant because AI Bank already uses `did:key`-style bare Ed25519; UCAN's `attenuation` (subset of capabilities) maps to AI Bank's future `max_amount`/`expiry` per-delegate cap [Source: `ucan.xyz` spec — UCAN as `issuer → audience` capability chain with proofs].
* **Macaroons / Biscuits** — bearer credentials with caveat attenuation (time, amount, target). Third-party caveats need discharge, first-party caveats self-contained. Overkill for FAL-2 where one key == one account.
* **libp2p `SignedEnvelope` / `PeerRecord`** — the `shared-registry.md §7.3` pattern `Seq + PeerId + addrs + sig` is itself a single-hop self-delegation (the peer delegates address authority to itself). Reuse the same `seq` monotonicity + `FilterBoth` gate if delegation is later added [Source: `libp2p specs` `SignedEnvelope`, `PeerRecord` + `shared-registry.md §7.3`].

All three confirm that delegation is well-understood and can be added additively (new table + new gossip topic) without rekeying existing `PeerId=AccountId` accounts, which is exactly why MVP should not block on it.

---

## 2. Transaction Schema: `{from, to, amount, nonce, sig, timestamp}`

### 2.1 Canonical `Transfer` Type (One Transfer = One Signed Unit)

```rust
// crate: ai-bank-types (shared by service, api, p2p, storage)
use libp2p_identity::PeerId;
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

const DOMAIN_TRANSFER: &[u8] = b"/ai-bank/1/transfer:";
const DOMAIN_BATCH: &[u8]    = b"/ai-bank/1/batch:";

/// Canonical account — MVP is PeerId directly (§1).
pub type AccountId = PeerId;

/// Amount in smallest unit — u64, see §4.
/// DUST_THRESHOLD and MAX_AMOUNT are consensus constants pinned in genesis/config.
pub type Amount = u64;

/// Monotonic per-sender counter — u64, 1-indexed; 0 is reserved for genesis.
/// Replaces ad-hoc "nonce" naming where needed (nonce == seq for MVP).
pub type Nonce = u64;

/// Milliseconds since Unix epoch (u64). Informational / display / reputation decay
/// input — NOT used for fork-choice ordering (see ledger-replication.md §3,
/// deterministic longest-valid-history with seq, not wall clock).
pub type TimestampMs = u64;

// ---- custom serde for PeerId as base58 string in JSON, bytes in CBOR ----
mod peer_id_serde {
    use super::*;
    use serde::{Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(id: &PeerId, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() { s.serialize_str(&id.to_base58()) }
        else { s.serialize_bytes(&id.to_bytes()) }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PeerId, D::Error> {
        if d.is_human_readable() {
            let s = String::deserialize(d)?;
            s.parse().map_err(D::Error::custom)
        } else {
            let b: Vec<u8> = Vec::deserialize(d)?;
            PeerId::from_bytes(&b).map_err(D::Error::custom)
        }
    }
    pub mod vec_serde {
        use super::*;
        pub fn serialize<S: Serializer>(ids: &Vec<PeerId>, s: S) -> Result<S::Ok, S::Error> {
            let strs: Vec<String> = ids.iter().map(|p| p.to_base58()).collect();
            strs.serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PeerId>, D::Error> {
            let strs = Vec::<String>::deserialize(d)?;
            strs.into_iter().map(|s| s.parse().map_err(D::Error::custom)).collect()
        }
    }
}

/// Single transfer — signed individually for fine-grained gossip validation.
/// For batch efficiency, multiple `Transfer`s can be wrapped in a `SignedBatch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    /// Sender — must equal recovered PeerId from `sig`; `== AccountId`.
    #[schemars(with = "String", description = "Sender PeerId base58 (12D3Koo…)")]
    #[serde(with = "peer_id_serde")]
    pub from: AccountId,

    /// Recipient — any PeerId (need not be online; balance credited on apply).
    #[schemars(with = "String", description = "Recipient PeerId base58")]
    #[serde(with = "peer_id_serde")]
    pub to: AccountId,

    /// Amount in smallest unit. Invariant: 0 < amount <= MAX_AMOUNT, amount >= DUST_THRESHOLD.
    #[schemars(range(min = 1), description = "Amount in smallest unit (u64)")]
    pub amount: Amount,

    /// Per-sender strictly monotonic. Next expected = max_seen[from] + 1.
    /// `0` is genesis-only; first transfer from any account is `1`.
    pub nonce: Nonce,

    /// Milliseconds since Unix epoch. Set by sender; accepted within ± skew window
    /// (e.g., 5 min) for display only — ordering uses `nonce`, not this.
    #[schemars(description = "Sender wall-clock ms since epoch; informational, not consensus")]
    pub timestamp: TimestampMs,

    /// Ed25519 signature over DOMAIN_TRANSFER || canonical_cbor(TransferWithoutSig).
    /// 64 bytes, stored as hex in JSON (human-readable) vs raw bytes in CBOR.
    #[schemars(with = "String", description = "Ed25519 sig hex(64B) over b\"/ai-bank/1/transfer:\"+cbor")]
    #[serde(with = "sig_hex_serde")]
    pub sig: [u8; 64],
}

// Helper: sig as hex string in JSON (human-readable), raw bytes in CBOR.
mod sig_hex_serde {
    use serde::{Serializer, Deserializer, de::Error};
    pub fn serialize<S: Serializer>(sig: &[u8;64], s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() { s.serialize_str(&hex::encode(sig)) }
        else { s.serialize_bytes(sig) }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8;64], D::Error> {
        if d.is_human_readable() {
            let hex = String::deserialize(d)?;
            let bytes = hex::decode(&hex).map_err(D::Error::custom)?;
            bytes.try_into().map_err(|_| D::Error::custom("sig must be 64 bytes"))
        } else {
            let bytes: Vec<u8> = Vec::deserialize(d)?;
            bytes.try_into().map_err(|_| D::Error::custom("sig must be 64 bytes"))
        }
    }
}

/// CBOR-signing view — same fields as Transfer without `sig`, deterministic encoding.
/// Produced via `cbor4ii` or `ciborium` with BTreeMap key ordering / canonical mode.
#[derive(Serialize)]
struct TransferWithoutSig<'a> {
    from: &'a AccountId,
    to: &'a AccountId,
    amount: Amount,
    nonce: Nonce,
    timestamp: TimestampMs,
}

impl Transfer {
    /// Bytes that are signed: DOMAIN || canonical_cbor(without sig).
    pub fn signing_bytes(&self) -> Vec<u8> {
        // `cbor4ii` or `ciborium` deterministic serialization of TransferWithoutSig
        let mut out = Vec::with_capacity(DOMAIN_TRANSFER.len() + 128);
        out.extend_from_slice(DOMAIN_TRANSFER);
        let view = TransferWithoutSig { from: &self.from, to: &self.to,
            amount: self.amount, nonce: self.nonce, timestamp: self.timestamp };
        // Option A: cbor4ii::serde::to_vec(&view) — deterministic when struct uses BTreeMap ordering
        // Option B: ciborium::into_writer(&view, &mut out) with canonical feature
        // Choose exactly one crate for the workspace (see §3.2).
        out.extend(cbor4ii::serde::to_vec(&view).expect("cbor encodable"));
        out
    }
    pub fn verify(&self) -> Result<(), VerifyError> {
        // 1. Recover VerifyingKey from `from` PeerId (try_from_protobuf → to VerifyingKey bytes)
        // 2. ed25519_dalek::VerifyingKey::verify(&self.signing_bytes(), &Signature::from_bytes(&self.sig))
        // 3. Enforce amount/nonce/timestamp bounds (see §4)
        todo!()
    }
}
```

**Field-by-field rationale:**

| Field | Type | Why this type | Validation rule | Source constraint |
|---|---|---|---|---|
| `from` | `PeerId` (`12D3Koo…` in JSON, `&[u8]` multihash bytes in CBOR) | ADR 0001 — PeerId is the canonical key; one type for transport + ledger (§1) | `sig` must verify against `from`'s public key; `from` must equal gossip envelope's `PeerId` if `MessageAuthenticity::Signed` is also used | `PeerId::is_public_key`, `PeerId::from_bytes` [Source: `peer_id.rs`] |
| `to` | `PeerId` | Same keyspace — any peer can receive, even offline (credit on replay) | Must be valid `PeerId` (parseable); may equal `from` — self-transfer is allowed but dust-checked (useful for testing, no-op otherwise) | Same |
| `amount` | `u64` smallest unit | Fits `SUPPLY` (see §4); `u128` accumulator only for sum check; JSON `number` without BigInt issues | `DUST_THRESHOLD <= amount <= MAX_AMOUNT && amount != 0`; additionally `balance[from] >= amount` on apply; `amount` checked at gossip `validate_messages` gate | ADR 0004 FAL-2 dust filter + `ledger-replication.md §4.3` |
| `nonce` | `u64` (`seq`) | Per-sender monotonic; O(1) replay protection; `u64` gives `1.8e19` transfers per account — never wraps in practice; wrap is a hard error (reject) | `nonce == expected_nonce[from]` where `expected = max_nonce[from] + 1`; `nonce == 0` rejected except genesis | `ledger-replication.md §4.2` per-sender seq |
| `sig` | `[u8;64]` Ed25519 | Deterministic, 64B, `ed25519_dalek::Signature::from_bytes` [Source: `ed25519_dalek 2.1 Signature`] | Must verify over `DOMAIN_TRANSFER || cbor(without_sig)`; domain separation prevents cross-protocol replay (ADR 0001) | ADR 0001 `b"/ai-bank/1/"` domain |
| `timestamp` | `u64` ms | Wall clock for UX/reputation-decay only; not in fork-choice ordering; allows `SystemTime::now().duration_since(UNIX_EPOCH).as_millis() as u64` on sender | Accepted if `abs(now - timestamp) < SKEW_MS` (e.g., 5 min or 1 h); future timestamps > skew are rejected; **ordering never uses this** — `nonce` wins | `ledger-replication.md §4.4` HLC discussion — plain `seq` suffices |

**What is *not* in `Transfer`:**

* `version` — lives on the `SignedBatch`/CBOR envelope (see §3, §7), not per-transfer; adding a byte per transfer wastes gossip bandwidth.
* `chain_id` / `network_id` — folded into the `DOMAIN_TRANSFER` constant (`b"/ai-bank/1/"` includes version). Changing domain = incompatible network (social fork / testnet), which is intentional.
* `fee` / `gas` — no fee market at FAL-2; all transfers are zero-fee fixed-supply moves. Deferred to FAL-3 if relay incentives are needed.
* `memo` / `metadata` — optional opaque field deferred; if added, it must be included in `signing_bytes` and length-capped (e.g., 256B) to prevent bloat; not in MVP to keep validation minimal.

### 2.2 Batch Wrapper (Wire & Storage Envelope)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedBatch {
    /// Wire/storage envelope version — 1 for MVP. Bumped on breaking CBOR shape change.
    #[schemars(description = "Envelope version, 1 for MVP")]
    pub version: u8,          // 1
    /// Author of the batch (== transfers[0].from when single-sender batch, or batch signer).
    #[serde(with = "peer_id_serde")]
    #[schemars(with = "String")]
    pub author: AccountId,
    /// Strictly monotonic per-author; gaps trigger request-response catch-up (§4.2).
    pub seq: u64,
    /// The transfers in this batch — all share `author` for single-sig batch,
    /// or each carries its own `sig` for multi-sender batch.
    pub transfers: Vec<Transfer>,
    /// Optional HLC / vector-clock metadata for debugging (not consensus) — sparse map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(description = "Optional causal metadata; not used for ordering")]
    pub vclock: BTreeMap<String, u64>, // PeerId base58 → max seq seen; String for JsonSchema
    /// Batch-level sig over DOMAIN_BATCH || cbor(batch_without_sig) — covers seq+vclock.
    /// For single-transfer batches, this sig is redundant with Transfer.sig but kept
    /// for uniform batch verification; verifiers check both.
    #[serde(with = "sig_hex_serde")]
    #[schemars(with = "String")]
    pub sig: [u8; 64],
}
```

* `gossipsub` publish payload is `cbor(SignedBatch)` (see `ledger-replication.md §2.1` `IdentTopic("/ai-bank/transfer/1.0.0")`, `MessageAuthenticity::Signed`, `ValidationMode::Strict`, `validate_messages(true)`).
* `request-response` CBOR `SyncResponse { batches: Vec<SignedBatch> }` reuses the same type (see `ledger-replication.md §2.3`).
* `redb` `BATCHES` table value is `cbor(SignedBatch)` (see §5).
* HTTP `POST /v1/transfer` accepts either `Transfer` (single) or `SignedBatch` (batch) — same `JsonSchema` reused for `rmcp` tool `transfer`.

### 2.3 `schemars` / `serde` Discipline

Every type that crosses the API boundary derives **both** `Serialize + Deserialize` and `JsonSchema` once, in `ai-bank-types` [Source: `schemars 1.1` — `#[derive(JsonSchema)]` generates JSON Schema 2020-12; `serde` is the Rust serialization framework `serde.rs`].

```rust
// Cargo.toml (ai-bank-types)
[dependencies]
serde = { version = "1", features = ["derive"] }
schemars = { version = "1.1", features = ["derive"] } # re-exports serde_json::Value schema
hex = "0.4"
libp2p-identity = { version = "0.2", features = ["peerid","ed25519","rand"] }
```

Discipline:

* `#[serde(deny_unknown_fields)]` on all ledger types — fail fast on version skew rather than silently ignoring new fields (forward compat is additive via `Option<T> + default` fields, never by dropping required fields).
* Custom `peer_id_serde` / `sig_hex_serde` as above — `is_human_readable()` branches so the **same struct** serializes as base58/hex strings in JSON (for `axum::Json` and `rmcp` `inputSchema`) and as raw bytes in CBOR (for gossip/storage) without a second DTO [Source: `serde` `Serializer::is_human_readable` — JSON is human-readable, CBOR/bincode are not].
* `schemars(with = "String")` on `PeerId` / `[u8;64]` fields so the generated JSON Schema shows `"type": "string"` with a description, not an opaque byte array — required for LLM tool-use accuracy (ADR 0003).
* Never derive `JsonSchema` on a CBOR-only helper (`TransferWithoutSig`) — only on the wire types agents actually construct.

---

## 3. CBOR vs JSON: Which Encoding Where

### 3.1 Decision Table (Primary Sources)

| Dimension | **JSON (`serde_json`)** | **CBOR (CBOR — RFC 8949, deterministic)** | Verdict for AI Bank |
|---|---|---|---|
| **Spec** | RFC 8259, text, human-debuggable, `{"from":"12D3Koo…","amount":100}` | RFC 8949, binary superset of JSON, supports bytes natively, `major type 2` for `&[u8]`, deterministic map-key ordering when canonical | JSON for human plane, CBOR for machine plane |
| **Where used** | `axum::Json<T>` HTTP handlers (ADR 0003 localhost `127.0.0.1`), `rmcp` `tools/list`+`tools/call` JSON-RPC 2.0 `inputSchema`/`outputSchema`, `curl` debugging, genesis file `genesis.json` (optional) | `gossipsub` publish payload (`IdentTopic("/ai-bank/transfer/1.0.0")`), `request-response` `cbor::Behaviour` codec, `redb` table values, `signing_bytes` input | **Both, one type** — see bridging below |
| **Bytes handling** | Must base64/hex encode `[u8;64]` sig — extra allocation + size (+33% for base64) | Native byte string — 64B stays 64B + 2B CBOR header | CBOR is the correct bytes carrier |
| **Determinism** | `serde_json` map-key order is insertion order (BTreeMap helps but whitespace/key-sort not guaranteed canonical) — **not safe as signing input** | CBOR deterministic encoding (RFC 8949 §4.2.1 — preferred serialization; `cbor4ii`/`ciborium` canonical mode sorts map keys by `len` then `lexicographic` bytes) — **safe as signing input** [Source: RFC 8949 §4.2 Core Deterministic Encoding, `cbor4ii` docs — canonical `Encode` with `BTreeMap`] | **CBOR only for `signing_bytes`** |
| **Schema** | `schemars::JsonSchema` generates JSON Schema — reused for `utoipa` OpenAPI AND for `rmcp` tool schemas per ADR 0003 §MCP Phase-2 | CBOR has CDDL (RFC 8610) but Rust uses `schemars` JSON Schema as the single schema source; CBOR is just a different `Serializer` for the same struct [Source: `schemars` — "JSON Schema 2020-12", CDDL is separate] | One `JsonSchema`, two serializers |
| **Crate surface** | `serde_json 1` (already pulled by `axum`, `schemars`, `rmcp`) | Candidates: `cbor4ii 0.3` (used by `libp2p-request-response::cbor`), `ciborium 0.2`, `serde_cbor 0.11` (maintenance fork), `minicbor` — comparison below | Pick **one** CBOR crate for the workspace; `cbor4ii` is the `libp2p` default |
| **Size / perf** | Larger (hex sig = 128 chars + JSON punctuation); human-readable | ~40% smaller for `Transfer` (binary PeerId + raw sig); faster encode/decode than JSON | Matters on `gossipsub` mesh (D=6 fan-out) |
| **Debuggability** | `curl http://127.0.0.1:PORT/v1/transfer \| jq` | `cbor2diag` / `cbor.me` for hex dumps; also expose JSON mirror at HTTP layer for debugging | Keep both |

**Decision: Dual-codec.** The same `Transfer`/`SignedBatch` structs are `Serialize`/`Deserialize` once and serialized **as CBOR** for P2P/storage/signing and **as JSON** for the localhost HTTP + MCP boundary. This is not two models — it is one model with two `serde` formats, bridged by `is_human_readable()`.

### 3.2 CBOR Crate Comparison (Current at 2026-09, via `crates.io` + `docs.rs`)

| Crate | Latest | Used by | Determinism | `serde` | MSRV / note | Verdict |
|---|---|---|---|---|---|---|
| **`cbor4ii 0.3.x`** (`cbor4ii::serde`) | 0.3.x | **`libp2p-request-response::cbor::Behaviour` is literally `cbor4ii::serde`** [Source: `docs.rs libp2p::request_response::cbor — "using cbor4ii::serde"`] | BTreeMap key sorted; struct fields in declaration order — deterministic when types use `BTreeMap` + `#[derive(Serialize)]` | Yes, `serde` feature | Pure Rust, `no_std` optional | **Recommended if using `libp2p-request-response` cbor** — zero codec mismatch |
| **`ciborium 0.2.x`** | 0.2.x | Iroh, common in Rust CBOR examples | Canonical mode via `ciborium::into_writer` with `Value` canonicalization (`ciborium` doc — canonical `Value::canonical()`) | Yes | Actively maintained, simple API | **Equally valid; choose if `libp2p` cbor is swapped to `ciborium`** — otherwise stick to `cbor4ii` for one codec |
| **`serde_cbor 0.11.x`** | 0.11.x | Historical default; fork `serde_cbor` (maintained by `serde-rs` org) | Not canonical by default; map-key order is insertion/BTreeMap but no length-first sort per RFC 8949 | Yes | Stable but not `libp2p`'s codec — would require dual codecs on P2P | Avoid — codec mismatch with `libp2p-request-response` |
| **`minicbor 0.24`** | 0.24+ | High-perf, `no_std` | Requires manual `Encode`/`Decode` impls, not `serde`-derived — incompatible with `schemars` single-type goal | No (own `Encode`/`Decode` traits) | Fastest, but adds a second derive macro | Reject — breaks shared-types crate |

**Concrete wiring:**

```toml
# Cargo.toml (workspace root)
[dependencies]
libp2p = { version = "0.56", features = ["tokio","tcp","quic","dns","noise","yamux","identify","kad","gossipsub","request-response","relay","dcutr","autonat","mdns"] }
cbor4ii = { version = "0.3", features = ["serde", "use_std"] }  # aligns with libp2p cbor codec
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = { version = "1.1", features = ["derive"] }
ed25519-dalek = "2"
blake3 = "1"
hex = "0.4"
redb = "4.1"  # or 3.1, stable format since 1.0
```

```rust
// P2P send — same bytes as storage
let batch = SignedBatch { version: 1, author, seq, transfers, .. };
let cbor_bytes = cbor4ii::serde::to_vec(&batch)?; // for gossipsub publish
// libp2p-request-response cbor::Behaviour does this internally for SyncRequest/SyncResponse
behaviour.gossipsub.publish(topic, cbor_bytes.clone())?;
db_batch_table.insert(seq, cbor_bytes.as_slice())?; // redb (see §5)

// HTTP respond — same type, different serializer
async fn get_transfer(Json(payload): Json<Transfer>) -> Json<Transfer> { Json(payload) }
// `axum::Json` uses `serde_json::to_vec` internally (human-readable branch)

// Signing — CBOR canonical of TransferWithoutSig lives inside signing_bytes() above
```

**Canonical rule:** Field order in `struct Transfer` declaration is the CBOR encoding order (serde serializes struct fields in declaration order). Do **not** reorder fields without bumping `SignedBatch.version` and documenting it as a breaking change (§7).

### 3.3 JSON-Only Exposure (Why Not CBOR-over-HTTP)

* LLMs produce JSON tool arguments via `tools/call` (`rmcp` JSON-RPC 2.0) — sending CBOR to an agent would require the agent to link a CBOR codec, which no OpenAI/Claude tool harness does today. Keep the localhost boundary JSON-only.
* CBOR diagnostics remain available: `GET /v1/debug/batch/{seq}/cbor` can return the raw `cbor4ii` bytes base64-encoded or as `application/cbor` for tooling; not the default.

---

## 4. Amounts, Dust, and Max: `u64` vs `u128`, Thresholds, Denomination

### 4.1 `u64` for Per-Account Balances, `u128` for Accumulator

| Choice | Range | JSON safe? | Storage (`redb::Table<&[u8], u64>`) | Supply check |
|---|---|---|---|---|
| **`u64` per account** | `0 .. 18_446_744_073_709_551_615` (1.8e19) units | `serde_json` Number handles up to `2^53-1` losslessly in JS, but Rust `u64` round-trips via string in `schemars` if `Number` is large — `axum` JSON handles it; LLM tool args as JSON number up to `9e15` are safe | `redb` native `u64` value (8B fixed) | Accumulate in `u128` to avoid wrap before `== SUPPLY` compare |
| `u128` per account | 3.4e38 — overkill, 16B per balance, JSON needs string (JS cannot parse) | No — requires JSON string for JS LLM clients, extra parsing | 16B per row | Unnecessary width |
| `u32` per account | 4.2e9 — may exhaust supply if denomination is 6 decimals and supply is 1e15 | Yes | 4B | Too small for any interesting supply at 6 decimals |

**Recommendation: `u64` per-account, `u128` accumulator for checks.**

Rationale — with 6 decimal places (1 credit = 1_000_000 units, like USDC), `SUPPLY = 1_000_000_000 * 1_000_000 = 1_000_000_000_000_000 (1e15)` fits in `u64` with 3 orders of magnitude headroom; even `SUPPLY = 1_000_000_000_000 * 1_000_000 = 1e18` still fits (`u64::MAX ≈ 1.84e19`). No ledger needs `u128` per-account. The only `u128` use is `balances.values().map(|b| b as u128).sum::<u128>() == SUPPLY as u128` to avoid wrapping the sum itself (see `ledger-replication.md §4.1`).

MSRV note: `u64` is native on all targets; `u128` widening is a single `mov` on x86_64/aarch64.

### 4.2 Constants: Where They Live, How They Are Enforced

```rust
// crate: ai-bank-types or ai-bank-config
/// Total supply in smallest unit — compiled into binary AND pinned in genesis artifact.
/// Must match genesis.json `supply` and `meta` table `supply`.
pub const SUPPLY: u64 = 1_000_000_000_000_000; // 1e9 credits * 1e6 = 1e15 — TBD by #12 grilling

/// Smallest transfer that gossip will propagate / ledger will apply.
/// Purpose: DoS damping (dust flood test, ADR 0006) + prevents state-bloat via 1-unit griefing.
pub const DUST_THRESHOLD: u64 = 100; // 0.0001 credits at 6 decimals

/// Largest single transfer allowed. Set to SUPPLY for MVP (any single transfer cannot exceed total).
/// Tightened later if FAL-3 wants per-tx caps.
pub const MAX_AMOUNT: u64 = SUPPLY;

/// Allowed timestamp skew for display-field validation (5 minutes).
pub const TIMESTAMP_SKEW_MS: u64 = 5 * 60 * 1000;

/// Non-consensus constants live in config/genesis; consensus checks (DUST, MAX, nonce, sum)
/// are hard-coded or hash-pinned so two nodes with different config still reject invalid batches.
```

**Consensus vs local policy (critical for FAL-2):**

* **Consensus (must be identical across all correct nodes)**: `SUPPLY`, `DUST_THRESHOLD`, `MAX_AMOUNT`, `DOMAIN`, CBOR field order, `u64` semantics. If two nodes disagree on dust threshold, one may `Validation::Accept` a 50-unit transfer and the other `Reject` — a soft fork. For MVP, pin all three in `genesis.json` (`{ supply, dust_threshold, max_amount, genesis_hash }`) and verify `blake3(canonical_genesis) == GENESIS_HASH` at startup. Changing any of them is a hard fork requiring a social `Checkpoint` (see `ledger-replication.md §3.4`).
* **Local policy (may differ)**: `TIMESTAMP_SKEW_MS`, rate limits (`gossipsub` `PeerScoreParams`, relay `Limit`), reputation decay params (#11). These do not affect `sum==SUPPLY` and can be tuned per node.

**Enforcement order (matches `ledger-replication.md §3.2` `verify_batch` fast-fail):**

```rust
fn validate_transfer(state: &StateView, t: &Transfer) -> Result<(), ValidationError> {
    if t.amount < DUST_THRESHOLD { return Err(ValidationError::Dust); }
    if t.amount > MAX_AMOUNT || t.amount == 0 { return Err(ValidationError::AmountOutOfRange); }
    if t.nonce == 0 { return Err(ValidationError::BadNonce); }
    let expected = state.nonce_of(&t.from).map_or(1, |n| n + 1);
    if t.nonce != expected {
        return if t.nonce <= expected - 1 { Err(ValidationError::Replay) }
               else { Err(ValidationError::Gap{ expected, got: t.nonce }) };
    }
    if t.timestamp > now_ms() + TIMESTAMP_SKEW_MS { return Err(ValidationError::FutureTimestamp); }
    // verify sig before balance checks (cheapest invalid to reject)
    t.verify()?;
    if state.balance_of(&t.from) < t.amount { return Err(ValidationError::InsufficientFunds); }
    // supply invariant checked at batch level after applying all transfers on a copy
    Ok(())
}
```

The amount + dust gate is placed **before** `report_message_validation_result(Validation::Accept)` in the gossipsub handler, so invalid amounts never flood the mesh and incur `P₄` penalty (see `ledger-replication.md §2.2` scoring).

### 4.3 Denomination Note (Blocked on #12, but Constrained Here)

`SUPPLY` and `decimals` are TBD by #12 genesis grilling. This doc constrains the shape:

* Store amounts as integer `u64` smallest units; **never float**.
* Recommended 6 decimals (like stablecoins) — `1 credit = 1_000_000 units` — balances render as `format!("{}.{:06}", amount / 1_000_000, amount % 1_000_000)` in HTTP `BalanceResponse`.
* JSON transport may render amounts as `u64` Number (safe to `9e15`) or as string if `SUPPLY` nears `9e18`; document which and keep it consistent across `axum` + `schemars` + `rmcp`. For MVP with `1e15`, JSON number is safe without BigInt.

---

## 5. Balance & Nonce Storage: `u64` Sums, `sum==SUPPLY`, `redb` Layout

### 5.1 State Model (Matches `ledger-replication.md §4`)

```rust
/// In-memory view of ledger state for `validate_transfer` / `apply_batch`.
/// Backed by redb tables (see §5.2). `total` is the fast-path sum.
#[derive(Debug, Clone)]
pub struct StateView {
    pub balances: BTreeMap<AccountId, u64>,  // mirroring redb BALANCES
    pub nonces:   BTreeMap<AccountId, u64>,  // mirroring redb NONCES
    pub total:    u64,                        // cached sum(balances) — must == SUPPLY
    pub tip_seq:  u64,                        // global or per-author seq of last applied batch
}

pub fn apply_batch_strict(state: &mut StateView, batch: &SignedBatch) -> Result<(), ApplyError> {
    // 1. Validate batch sig + each transfer via validate_transfer
    // 2. Copy-on-check: clone balances/nonces, mutate copy, then verify sum
    let mut next = state.clone();
    for t in &batch.transfers {
        validate_transfer(&next, t)?;
        *next.balances.entry(t.from.clone()).or_insert(0) -= t.amount;
        *next.balances.entry(t.to.clone()).or_insert(0) += t.amount;
        // BTreeMap entry API above is O(log n); see redb batch below for storage
        next.nonces.insert(t.from.clone(), t.nonce);
        // total stays SUPPLY by construction (move, not mint), but assert:
    }
    let sum: u128 = next.balances.values().map(|b| *b as u128).sum();
    if sum != SUPPLY as u128 { return Err(ApplyError::SupplyInvariant{ sum, expected: SUPPLY as u128 }); }
    debug_assert_eq!(next.total, SUPPLY);
    // 3. Commit copy to state + persist via redb write_txn (see §5.2)
    *state = next;
    Ok(())
}
```

**Supply invariant enforcement — two paths (from `ledger-replication.md §4.1`):**

* **Fast path (every batch):** maintain `state.total: u64` (initialized to `SUPPLY` at genesis) and assert `state.total == SUPPLY` after apply. Since transfers are moves (`from -= amount; to += amount`), `total` is conserved by construction; the check is `debug_assert!` / `assert!` that no code path minted/burned.
* **Audit path (every N batches + on startup + on checkpoint):** full-scan `balances.values().map(|b| *b as u128).sum::<u128>() == SUPPLY as u128` iterating the `redb` `BALANCES` table via `table.range(..)` — O(accounts), ~80µs for 10k accounts on `redb` (see `ledger-replication.md §4.1` per-batch cost table).

### 5.2 `redb` Table Layout (Recommended — Stable Format Since 1.0)

`redb` is the recommended ledger store per `ledger-replication.md §7` (pure Rust, copy-on-write B+-trees, ACID MVCC, **stable file format since 1.0 (2023-06-16)** [Source: `docs.rs/redb 3.1.0 — file format stable since 1.0`][Source: `crates.io redb 4.2.0`]).

```rust
use redb::{Database, TableDefinition, ReadableTable, ReadableTableMetadata};

/// redb is single-writer, many-readers (MVCC) — matches ledger's single-writer-per-node
/// append model (§7.1 of ledger-replication). One Database handle shared across swarm + axum.

/// Key: PeerId multihash bytes (PeerId::to_bytes()), Value: u64 balance (native).
const BALANCES: TableDefinition<&[u8], u64> = TableDefinition::new("balances");
/// Key: PeerId bytes, Value: u64 highest applied nonce/seq for that account.
const NONCES:   TableDefinition<&[u8], u64> = TableDefinition::new("nonces");
/// Key: u64 global seq (if linearized) OR composite (PeerIdBytes ++ be64(seq)) — choose one
/// and freeze it. Value: CBOR bytes of SignedBatch (opaque &[u8] to redb).
const BATCHES:  TableDefinition<u64, &[u8]> = TableDefinition::new("batches");
/// Meta table — single-row config + versioning (see §7).
/// Keys are &str ("schema_version","supply","dust_threshold","max_amount","genesis_hash","checkpoint_seq")
/// Values are CBOR/bytes. String keys are tiny and human-grep-able.
const META:     TableDefinition<&str, &[u8]> = TableDefinition::new("meta");

pub fn open_db(path: &str) -> Result<Database, redb::Error> {
    let db = Database::create(path)?; // e.g., "~/.ai-bank/ledger.redb"
    // Create tables on first open (idempotent)
    let wt = db.begin_write()?;
    { wt.open_table(BALANCES)?; wt.open_table(NONCES)?; wt.open_table(BATCHES)?; wt.open_table(META)?; }
    wt.commit()?;
    Ok(db)
}

pub fn apply_batch_atomic(db: &Database, batch: &SignedBatch, preview_state: &StateView) -> Result<(), redb::Error> {
    // preview_state was already validated via apply_batch_strict (copy-on-check)
    // Now persist atomically: balances + nonces + batches + meta checkpoint in one write_txn
    let cbor_batch = cbor4ii::serde::to_vec(batch).expect("cbor encodable");
    let txn = db.begin_write()?;
    {
        let mut bal = txn.open_table(BALANCES)?;
        let mut non = txn.open_table(NONCES)?;
        let mut batches = txn.open_table(BATCHES)?;
        let mut meta = txn.open_table(META)?;

        for t in &batch.transfers {
            let from_key = t.from.to_bytes();
            let to_key = t.to.to_bytes();
            let from_bal = bal.get(from_key.as_slice())?.map(|v| v.value()).unwrap_or(0);
            let to_bal   = bal.get(to_key.as_slice())?.map(|v| v.value()).unwrap_or(0);
            bal.insert(from_key.as_slice(), (from_bal - t.amount))?;
            bal.insert(to_key.as_slice(),   (to_bal + t.amount))?;
            non.insert(from_key.as_slice(), t.nonce)?;
        }
        batches.insert(batch.seq, cbor_batch.as_slice())?;
        // optional: meta.insert("tip_seq", &batch.seq.to_be_bytes())?;
        // meta.insert respects same atomic commit — crash cannot advance nonce without balance
    }
    txn.commit()?; // ACID — durability fsyncs
    Ok(())
}
```

**Key design points:**

* **Atomicity:** One `write_txn` covers `balances + nonces + batches` — crash cannot advance `nonces` without persisting `balances` (see `ledger-replication.md §4.2` persistence of nonce map).
* **Read concurrency:** `axum` handlers and reputation (#11) do `db.begin_read()?.open_table(BALANCES)?.get(k)` concurrently with the writer — `redb` MVCC allows many readers without blocking one writer [Source: `redb docs — MVCC, one writer + concurrent readers`].
* **Key encoding for `BATCHES`:** If linearized global `seq: u64`, key is `u64` (simple). If per-author `seq`, key must be composite `(PeerIdBytes, u64)` to avoid cross-author collision; encode as `blake3(PeerId_bytes || be64(seq))` truncated or as `TableDefinition<&[u8], &[u8]>` with composite bytes `peer_bytes + be64(seq)`. Choose one scheme and freeze in `META.schema_version`.
* **Genesis bootstrap:** First `write_txn` inserts genesis allocation table (see §4 / #12). Verify `sum(balances) == SUPPLY` before first commit, then set `meta("genesis_hash", blake3(canonical_genesis))`.
* **Why not `sled` for MVP:** `sled` is lock-free/Bw-tree, better for many concurrent writers, but ledger apply is single-writer sequentially; `sled` warns `if reliability is your primary constraint, use SQLite. sled is beta` and `on-disk format will change before 1.0` with manual `export` migration [Source: `docs.rs/sled 0.34.7 — beta warning, format instability`]. `redb`'s format stability since 1.0 and active maintenance (`4.2.0` 2026-08, 1.17M downloads/mo [Source: `crates.io redb`]) justifies it per `ledger-replication.md §7.1`.

### 5.3 Nonce Map — Replay Protection Details

* **Type:** `BTreeMap<AccountId, u64>` in-memory, `redb` `NONCES` table on disk.
* **Semantics:** `expected_nonce[peer] = stored[peer].map_or(1, |n| n+1)`. First transfer from a never-seen account is `nonce=1` (or `0` if `0` is reserved for genesis credit — pick one and document; `1` is recommended to keep `0` as sentinel for "never transferred").
* **Gap handling:** `nonce > expected` → `ValidationError::Gap`; buffer or trigger `request-response` catch-up `SyncRequest{ since_seq: expected-1 }` to fetch missing batch (see `ledger-replication.md §2.3` anti-entropy).
* **Equivocation:** two batches with same `nonce` but different payload → first-accept wins, second is `EquivocationProof` gossipped on `/ai-bank/evidence/1.0.0` (see `ledger-replication.md §3.3`).
* **Persistence of nonce map:** same transaction as balances (§5.2 above) — atomic.

---

## 6. API Surface Alignment with ADR 0003: `axum::Json<T>` + `schemars` Shared with `rmcp`

### 6.1 ADR 0003 Contract (Recap)

> LLM agents interact via localhost-only `axum` `Json<T>` on `127.0.0.1`; an MCP adapter (`rmcp 3.2` `#[tool]`, `transport-io` + `transport-streamable-http-server`) is Phase-2 sharing the **same service layer**; OpenAPI via `utoipa`/`schemars` [Source: `docs/adr/0003-agent-interface.md:1`].

Constraint: unknown MCP tool returns JSON-RPC `-32602`, not `isError:true`; avoid tool bloat via `tool_search`; bind `TcpListener::bind("127.0.0.1:0")` (ephemeral) [Source: ADR 0003 consequences].

### 6.2 Shared-Types Crate Pattern (One Source of Truth)

```text
crates/
  ai-bank-types/   # ← this doc specifies this crate's contents
    src/lib.rs     # Transfer, SignedBatch, BalanceRequest/Response, etc. — all with Serialize+Deserialize+JsonSchema
  ai-bank-service/ # pure service layer: fn transfer(state, Transfer) -> Result<Receipt>, fn balance(state, PeerId) -> u64
  ai-bank-api/     # axum handlers: Json<Transfer> → service::transfer
  ai-bank-mcp/     # rmcp #[tool_router] handlers → same service::transfer/service::balance
  ai-bank-p2p/     # libp2p swarm: cbor(SignedBatch) ↔ service::apply_batch
  ai-bank-storage/ # redb wrapper (see §5.2)
```

```rust
// crates/ai-bank-types/src/lib.rs
pub mod transfer;   // Transfer, SignedBatch, TransferError
pub mod api;        // BalanceRequest, BalanceResponse, TransferResponse, ErrorEnvelope
pub mod genesis;    // Genesis { supply, dust_threshold, max_amount, balances: BTreeMap<String,u64>, sig }

// All types in `transfer` and `api` derive Serialize + Deserialize + JsonSchema exactly once.
```

### 6.3 `axum` Handler Side

```rust
// crates/ai-bank-api/src/handlers.rs
use axum::{Json, extract::State, http::StatusCode};
use schemars::JsonSchema;
use ai_bank_types::{Transfer, api::*};
use ai_bank_service as service;

#[derive(serde::Deserialize, JsonSchema)]
pub struct BalanceQuery { pub peer: String } // PeerId base58

/// GET /v1/balance?peer=12D3Koo…
/// Handler signature uses axum::Json for request/response (ADR 0003).
async fn get_balance(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<BalanceQuery>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let peer: PeerId = q.peer.parse().map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorEnvelope{ code: 400, message: e })))?;
    let bal = service::balance(&state, &peer); // reads redb (begin_read) — non-blocking
    Ok(Json(BalanceResponse { peer: peer.to_base58(), balance: bal, supply: SUPPLY }))
}

/// POST /v1/transfer  — body: Transfer (JSON), sig included.
/// Also accepts SignedBatch for batch; axum's Json<T> try_from JSON via serde_json.
async fn post_transfer(
    State(state): State<AppState>,
    Json(transfer): Json<Transfer>,
) -> Result<Json<TransferResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    // Validate (sig, dust, nonce, funds, sum) — same validate_transfer as p2p handler.
    // On success: apply_batch_atomic (redb write_txn), then gossipsub publish cbor(SignedBatch).
    match service::apply_transfer(&state, transfer).await {
        Ok(receipt) => Ok(Json(TransferResponse { status: "accepted".into(), receipt })),
        Err(e) if e.is_validation() => Err((StatusCode::UNPROCESSABLE_ENTITY, Json(ErrorEnvelope::from(e)))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorEnvelope::from(e)))),
    }
}

pub fn router(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/v1/balance", axum::routing::get(get_balance))
        .route("/v1/transfer", axum::routing::post(post_transfer))
        // OpenAPI via utoipa derived from JsonSchema — utoipa 5 can ingest schemars 1.1 schemas
        // .merge(utoipa::OpenApi::from(...)) or `aide` for axum-native derive
}
```

* `axum 0.8 + tokio` per ADR 0003 [Source: `docs/adr/0003-agent-interface.md` — `axum 0.8` + `tokio`].
* `utoipa` (or `aide`) consumes the same `JsonSchema` to emit OpenAPI — no second annotation language [Source: `utoipa` — `#[derive(OpenApi)]` with `#[derive(ToSchema)]` which is `schemars`-compatible; `aide` is `axum`-native alternative].
* `schemars` `JsonSchema` is the single schema source; changing `Transfer` fields automatically updates both `GET /v1/balance` docs and `rmcp` `inputSchema`.

### 6.4 `rmcp` (`modelcontextprotocol-rust-sdk`) Side — Same Types

```rust
// crates/ai-bank-mcp/src/tools.rs
use rmcp::{ServerHandler, tool, tool_router, model::*};
use schemars::JsonSchema;
use ai_bank_types::{Transfer, api::*};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TransferToolInput {
    #[schemars(description = "Sender PeerId (12D3Koo…); must match signing key")]
    from: String,
    #[schemars(description = "Recipient PeerId (12D3Koo…)")]
    to: String,
    #[schemars(description = "Amount in smallest unit, >= DUST_THRESHOLD (100)")]
    amount: u64,
    #[schemars(description = "Per-sender nonce; next expected = last_nonce+1")]
    nonce: u64,
    #[schemars(description = "Ed25519 sig hex over b\"/ai-bank/1/transfer:\"+cbor; 128 hex chars")]
    sig: String,
    #[schemars(description = "Sender wall-clock ms; informational")]
    timestamp: u64,
}

#[tool_router]
impl BankTools {
    #[tool(description = "Transfer credits between accounts. Validates dust, nonce, balance, supply invariant.")]
    async fn transfer(&self, input: TransferToolInput) -> Result<CallToolResult, rmcp::Error> {
        // Convert tool input → Transfer (parse PeerId base58, hex sig → [u8;64])
        // Reuse the SAME validation as axum and p2p — single service::apply_transfer
        let transfer = input.into_transfer()?;
        let receipt = service::apply_transfer(&self.state, transfer).await
            .map_err(|e| rmcp::Error::invalid_params(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::json(receipt)?]))
    }

    #[tool(description = "Get balance for a PeerId. Returns { peer, balance, supply }.")]
    async fn get_balance(&self, peer: String) -> Result<CallToolResult, rmcp::Error> {
        let receipt = service::balance(&self.state, &peer.parse()?);
        Ok(CallToolResult::success(vec![Content::json(receipt)?]))
    }
}
```

* `rmcp 3.2` per ADR 0003 — `#[tool]` / `#[tool_router]` derive `inputSchema`/`outputSchema` from `schemars::JsonSchema` automatically; unknown tool returns JSON-RPC `-32602` [Source: `docs/adr/0003-agent-interface.md` — `rmcp 3.2 #[tool]`/`#[tool_router]`, `-32602`].
* **No DTO duplication:** The ` TransferToolInput ` fields mirror `Transfer` (with `String`/`hex` for `PeerId`/`sig`) so `schemars` generates the same constraints (`range(min=1)` for `amount`, base58 pattern). Alternatively, derive `JsonSchema` directly on `Transfer` and use it as the `#[tool]` param type (`async fn transfer(&self, t: Transfer)`) — `rmcp` supports `JsonSchema`-derived params as `inputSchema`. Keeping `Transfer` as the direct tool param is preferred (one type), with `peer_id_serde` handling base58 in JSON so agents see `"from": "12D3Koo…"` unchanged.

**Tool description discipline (ADR 0003 — agent accuracy):** Every `#[tool(description = "…")]` must state `amount` units (`smallest unit`), dust threshold, `nonce` monotonicity, and sig domain — LLM tool-use accuracy depends on description quality.

### 6.5 Transport-Blind Service Layer (One Validation Path)

Both planes call the same `service::{validate_transfer, apply_batch_atomic, balance}` — the invariant is enforced exactly once. The only difference is serialization:

```text
agent --JSON--> axum Json<Transfer>  ─┐
agent --JSON--> rmcp tools/call      ─┤
p2p  --CBOR--> gossipsub/req-res     ─┤─→ service::validate → redb write_txn → gossipsub publish
storage --CBOR--> redb BATCHES       ─┘
```

This matches ADR 0002 consequence `Service layer stays transport-blind: localhost HTTP handler and P2P handler both call service::{balance,transfer}` and `ledger-replication.md §5` event-loop sketch.

---

## 7. Persistence Format & Versioning for Ledger Log

### 7.1 File Layout & `redb` Format Stability

* **File:** single `ai-bank.redb` per node, default path `~/.ai-bank/ledger.redb` (or `$XDG_DATA_HOME/ai-bank/ledger.redb` on Linux) — operator responsibility per ADR 0001 file-discipline.
* **Format:** `redb` stable since 1.0 (2023-06-16); `4.x` major = API break only, file upgrade is in-place and forward-readable [Source: `docs.rs/redb 3.1.0 — stable file format since 1.0; reasonable effort to provide upgrade path`][Source: `crates.io redb 4.2.0`].
* **Backup:** `Database::compact()`-style or file copy under `begin_read` — document `cp ledger.redb ledger.backup.redb` for operators; redb's copy-on-write makes read-locked copy safe.

### 7.2 Schema Versioning (Two Levels)

**Level 1 — `META.schema_version: u32` (DB-level):**

```rust
const SCHEMA_VERSION: u32 = 1; // bump on table addition/rename, key-encoding change, genesis shape change

fn ensure_schema(db: &Database) -> Result<(), SchemaError> {
    let rt = db.begin_read()?; let meta = rt.open_table(META)?;
    let stored = meta.get("schema_version")?.map(|v| u32::from_be_bytes(v.value().try_into().unwrap())).unwrap_or(0);
    drop(rt);
    match stored.cmp(&SCHEMA_VERSION) {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Less  => migrate(db, stored, SCHEMA_VERSION),
        std::cmp::Ordering::Greater => Err(SchemaError::FutureVersion{ stored, current: SCHEMA_VERSION }),
    }
}
```

* Stored as `&[u8]` = `u32::to_be_bytes(SCHEMA_VERSION)`; checked on startup before any read/write. Unknown future version → error (not silent read) + prompt operator to upgrade binary.
* Migrations are `write_txn` transforms: `read old_table → transform → write new_table → remove old_table → bump META.schema_version`. Keep migrations forward-only (no downgrade); each migration is a pure function on the `redb` snapshot so it can be tested via `cargo test` with a fixture `ledger-v1.redb` copied from `tests/fixtures/`.

**Level 2 — CBOR `SignedBatch.version: u8` (value-level):**

* `SignedBatch.version = 1` for MVP. Every `BATCHES` value and every gossipsub payload carries its own version byte as the first struct field, so an upgraded node can still decode old batches written by peers or on disk.
* Encoding discipline: `#[serde(deny_unknown_fields)]` on `Transfer`/`SignedBatch` v1 — old binary rejects unknown fields from new peers (fail loud). For additive evolution (new optional field), add `#[serde(default)] pub memo: Option<String>` with `default` and keep `deny_unknown_fields` — new field ignored by old binary would still be `Reject` at gossip validation, so deploys must be coordinated via `Checkpoint` pinning a version cutover. Alternative is `#[serde(default)]` + removing `deny_unknown_fields` to tolerate forward compat — choose one per release and document in `MIGRATIONS.md`.

**Level 3 (implicit) — CBOR struct field order:**

* `Transfer` field declaration order is the canonical CBOR order. Reordering is a **breaking change** requiring `schema_version` + `SignedBatch.version` bump and a `Checkpoint` social fork (see `ledger-replication.md §3.4`) — not just a code refactor.

### 7.3 Serde Evolution Rules (Additive-Only for Ledger Types)

For `Transfer`/`SignedBatch`/`Genesis` (consensus types):

1. **Never rename a required field** without a `version` bump.
2. **Never change a field type** (e.g., `amount: u64 → u128`) without a bump.
3. **Add new optional fields as `Option<T>` with `#[serde(default)]`** — old nodes decode new batches as `None`; new nodes handle both.
4. **Deprecate by ignoring** — keep the old field as `Option<T>` but stop writing it; document removal after `N` checkpoints.
5. **Keep `PeerId`/`sig` serde helpers stable** — changing `peer_id_serde` base58 ↔ multibase would fork the network.

For `BalanceResponse`/`ErrorEnvelope`/`TransferResponse` (API-only types): looser — fields can be added freely since they are local JSON and never go into `signing_bytes` or `sum==SUPPLY`.

### 7.4 Gossip Envelope Versioning (Protocol Idents)

* `gossipsub` topic includes version: `"/ai-bank/transfer/1.0.0"` per `ledger-replication.md §2.1`. Changing `Transfer` shape → new topic `"/ai-bank/transfer/2.0.0"` with dual-subscribe during migration.
* `request-response` protocol protocol name: `"/ai-bank/sync/1.0.0"` similarly versioned.

### 7.5 Genesis & Checkpoint Pinning Interaction

* `genesis.json` / `genesis.cbor` carries `schema_version: 1` + `supply + dust_threshold + max_amount` + `history_hash: blake3(canonical_genesis)` — hash-pinned at compile time and in `META.genesis_hash` [Source: `ledger-replication.md §4.1` genesis artifact].
* `Checkpoint{ seq, history_hash, sigs }` pins `history_hash` which implicitly pins the schema version at that seq — fork-choice must verify `checkpoint.history_hash` extends a history whose `SignedBatch.version` sequence is compatible (no downgrade across checkpoint).

### 7.6 Migration Sketch (Reversible via Fork)

```rust
fn migrate(db: &Database, from: u32, to: u32) -> Result<(), SchemaError> {
    assert_eq!((from, to), (1, 2), "MVP only has one migration path");
    let txn = db.begin_write()?;
    {
        // Example: v2 adds `memo: Option<String>` to Transfer — no data rewrite needed because
        // CBOR with #[serde(default)] on new field decodes old bytes as None.
        // Only bump meta:
        let mut meta = txn.open_table(META)?;
        meta.insert("schema_version", &2u32.to_be_bytes()[..])?;
        meta.insert("migration_note", br#"v1->v2: Transfer.memo Option<String> default None"#)?;
    }
    txn.commit()?;
    Ok(())
}
// Full table rewrite example (v2→v3 with key encoding change):
// read_table(BATCHES).range(..).for_each(|(k,v)| new_table.insert(encode_new_key(k), transform(v)));
```

* Test: commit a fixture `tests/fixtures/ledger-v1.redb` with known transfers, run `ensure_schema` → `assert_eq!(schema_version,2)` → `assert_eq!(sum(balances), SUPPLY)`.

---

## 8. Canonical Types — Complete MVP Module (Copy-Paste Source)

```rust
//! ai-bank-types — single source of truth for all subsystems.
//! Every struct derives Serialize + Deserialize + JsonSchema exactly once.
//! CBOR for P2P/storage (cbor4ii), JSON for HTTP/MCP (serde_json) via is_human_readable().

use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use libp2p_identity::PeerId;

pub use Amount as Amount; // re-export
pub const SUPPLY: u64 = 1_000_000_000_000_000; // TBD #12
pub const DUST_THRESHOLD: u64 = 100;
pub const MAX_AMOUNT: u64 = SUPPLY;
pub const SCHEMA_VERSION: u32 = 1;
pub const BATCH_VERSION: u8 = 1;

// ---- PeerId / sig serde helpers (see §2.1 for bodies) ----
mod peer_id_serde; mod sig_hex_serde;

/// Transfer — §2.1
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    #[serde(with = "peer_id_serde")] #[schemars(with = "String")] pub from: PeerId,
    #[serde(with = "peer_id_serde")] #[schemars(with = "String")] pub to: PeerId,
    pub amount: u64,
    pub nonce: u64,
    pub timestamp: u64,
    #[serde(with = "sig_hex_serde")] #[schemars(with = "String")] pub sig: [u8;64],
}

/// SignedBatch — §2.2
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedBatch {
    pub version: u8,
    #[serde(with = "peer_id_serde")] #[schemars(with = "String")] pub author: PeerId,
    pub seq: u64,
    pub transfers: Vec<Transfer>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub vclock: std::collections::BTreeMap<String, u64>,
    #[serde(with = "sig_hex_serde")] #[schemars(with = "String")] pub sig: [u8;64],
}

/// Genesis artifact — §4.1 / #12
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    pub schema_version: u32,
    pub supply: u64,
    pub dust_threshold: u64,
    pub max_amount: u64,
    /// Allocation table — PeerId base58 → balance. Sum must == supply.
    pub balances: std::collections::BTreeMap<String, u64>,
    /// Blake3 hash of canonical CBOR/JSON of Genesis without `sig` (pin at compile time).
    #[serde(with = "sig_hex_serde")] #[schemars(with = "String")] pub history_hash: [u8;32],
    #[serde(with = "sig_hex_serde")] #[schemars(with = "String")] pub sig: [u8;64],
}

/// API — GET /v1/balance
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BalanceResponse { pub peer: String, pub balance: u64, pub supply: u64 }

/// API — POST /v1/transfer 200
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TransferResponse { pub status: String, pub seq: u64, pub tx_hash: String }

/// API — error envelope (any 4xx/5xx)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorEnvelope { pub code: u16, pub message: String, pub retryable: bool }

/// Sync RPC types — request-response CBOR (ledger-replication.md §2.3)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SyncRequest { pub since_seq: u64, pub limit: u16 }
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SyncResponse { pub batches: Vec<SignedBatch> }
```

*All primary-type decisions in this file are diffable against `ledger-replication.md` §4/§5 placeholder names — this section is the authoritative handoff.*

---

## 9. Crate Map & Verification (Current at 2026-09, via `crates.io` + `docs.rs` + Specs)

| Crate | Latest stable | MSRV / note | Needed for data model | Source |
|---|---|---|---|---|
| `libp2p-identity` | **0.2.x** (split from `libp2p-core` 2023) | MSRV 1.83 via umbrella | `PeerId`, `Keypair`, `PublicKey::verify`, `is_public_key` | [`crates.io libp2p-identity`](https://crates.io/crates/libp2p-identity) + [`docs.rs libp2p_identity`](https://docs.rs/crate/libp2p-identity/latest) + `specs/peer-ids` |
| `ed25519-dalek` | **2.1.0** | MSRV 1.65+ | `SigningKey`/`VerifyingKey`/`Signature::from_bytes`, `sign(domain||cbor)` | [`crates.io ed25519-dalek 2.1.0`](https://crates.io/crates/ed25519-dalek) + [RFC 8032] |
| `libp2p` umbrella | **0.56.0** (2025-06-28, bundles kad/gossipsub/req-res) | 1.83.0 | Owns codec choice for P2P | [`libp2p.io releases 2025-06-28`](https://libp2p.io/releases/2025-06-28-rust-libp2p/) |
| `cbor4ii` | **0.3.x** | `use_std`, `serde` | Canonical CBOR for P2P/storage/signing (same as `libp2p-request-response::cbor`) | [`docs.rs libp2p::request_response::cbor — "using cbor4ii::serde"`](https://docs.rs/libp2p/latest/libp2p/request_response/cbor/index.html) |
| `ciborium` | **0.2.x** | alt CBOR if swapping off `cbor4ii` | Same role, choose one | [`crates.io ciborium`](https://crates.io/crates/ciborium) |
| `serde` / `serde_json` | **1.x** | Ubiquitous | `Serialize`/`Deserialize`, JSON for HTTP/MCP | [`serde.rs`](https://serde.rs) + [`docs.rs serde_json`](https://docs.rs/crate/serde_json/latest) |
| `schemars` | **1.1.x** | JSON Schema 2020-12 | `#[derive(JsonSchema)]` — single schema for `utoipa`+`rmcp` | [`docs.rs schemars 1.1`](https://docs.rs/crate/schemars/latest) |
| `axum` | **0.8.x** | MSRV 1.75+ | `Json<T>` extractor, `127.0.0.1` router | [`docs.rs axum 0.8`](https://docs.rs/crate/axum/latest) + ADR 0003 |
| `rmcp` | **3.2.x** | `#[tool]`/`#[tool_router]`, `transport-io`/`streamable-http` | MCP adapter reusing same `JsonSchema` types | [`crates.io rmcp`](https://crates.io/crates/rmcp) + ADR 0003 |
| `utoipa` / `aide` | `5.x` / `0.14+` | OpenAPI from `JsonSchema` | HTTP docs from same derives | [`docs.rs utoipa`](https://docs.rs/crate/utoipa/latest) |
| `redb` | **4.2.0** (2026-08) / **3.1.0** (2025-09) — **format stable since 1.0** | Pure Rust, `no_std` compat | Local ledger log — `balances`/`nonces`/`batches`/`meta` | [`docs.rs redb 3.1.0`](https://docs.rs/crate/redb/3.1.0) + [`crates.io redb`](https://crates.io/crates/redb) |
| `sled` | **0.34.7** (2021-09-12) stable; `1.0.0-alpha.124` pre | Beta warning, format unstable pre-1.0 | Deferred alternative | [`docs.rs sled 0.34.7`](https://docs.rs/crate/sled/0.34.7) |
| `blake3` | **1.x** | Hash for `history_hash`, `signing_bytes` pinning | `blake3::hash(canonical_bytes)` | [`crates.io blake3`](https://crates.io/crates/blake3) |
| `hex` | **0.4.x** | Hex for JSON sig display | `hex::encode/decode` in `sig_hex_serde` | [`crates.io hex`](https://crates.io/crates/hex) |
| `multihash` / `bs58` | `0.19` / `0.5` | PeerId display | Already via `libp2p-identity` | ADR 0001 `multihash 0.19`, `bs58 0.5` |

### Verification Commands (Copy-Paste)

```bash
# 1) crates.io metadata (no clone needed)
cargo search libp2p-identity --limit 3
cargo search cbor4ii --limit 3
cargo search schemars --limit 3
cargo search redb --limit 3
cargo info libp2p-identity@0.2.13
cargo info schemars@1.1.0
cargo info redb@4.1.0
cargo info cbor4ii@0.3.2

# 2) local pin check after Cargo.toml edit
cargo tree | grep -E "libp2p-identity|schemars|redb|cbor4ii|ed25519-dalek|axum|rmcp"
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="redb") | {version, rust_version}'
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="schemars") | {version, rust_version}'

# 3) docs.rs as source of truth per-field
# https://docs.rs/libp2p-identity/latest/libp2p_identity/struct.PeerId.html
# https://docs.rs/libp2p-identity/latest/libp2p_identity/struct.PublicKey.html  # verify
# https://docs.rs/schemars/latest/schemars/trait.JsonSchema.html
# https://docs.rs/serde/latest/serde/trait.Serializer.html#method.is_human_readable
# https://docs.rs/crate/redb/3.1.0  # stable format note + TableDefinition
# https://docs.rs/crate/sled/0.34.7 # beta warning
# https://docs.rs/axum/latest/axum/struct.Json.html
# https://www.rfc-editor.org/rfc/rfc8949  # CBOR deterministic encoding §4.2
# https://www.rfc-editor.org/rfc/rfc8610  # CDDL (CBOR schema, if needed later)

# 4) spec as authority for PeerId encoding
# https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md
# https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md
```

Pin MVP to **`libp2p 0.56 + libp2p-identity 0.2 + cbor4ii 0.3 + serde 1 + schemars 1.1 + axum 0.8 + rmcp 3.2 + redb 3.1/4.x` (MSRV 1.83)**. `redb 4.x` is in-place upgrade from `3.x` (stable file format, major = API break only) — verify with `cargo tree | grep redb`.

---

## 10. Recommendations — Decision-Ready Summary for #10

1. **Account ID = PeerId (bytes on ledger/storage/CBOR, base58 in JSON).** No derived key, no delegation cert for MVP. One `Keypair` per account, same key signs handshakes and transfers over `b"/ai-bank/1/transfer:"`. Defer HD derivation to FAL-3 if many-accounts-per-node is needed, and UCAN-style `Delegation` to FAL-3 if agent-process ≠ node-operator. Handoff to #11: reputation keys off `PeerId`.

2. **Transfer is a flat `#[derive(Serialize,Deserialize,JsonSchema)]` struct** with `from,to: PeerId, amount: u64, nonce: u64, timestamp: u64, sig: [u8;64]` — `sig` over `b"/ai-bank/1/transfer:" || canonical_cbor(TransferWithoutSig)`. `amount` range is `DUST_THRESHOLD..=MAX_AMOUNT` (`100..=SUPPLY`). `nonce` is per-sender monotonic starting at `1`. `timestamp` is non-consensus informational within `±TIMESTAMP_SKEW_MS`. Deny unknown fields; keep field order frozen.

3. **CBOR for P2P+storage+signing, JSON for HTTP+MCP — one struct, two serializers.** Custom `peer_id_serde`/`sig_hex_serde` branch on `is_human_readable()` so JSON is base58/hex and CBOR is raw bytes. Use `cbor4ii 0.3` (matches `libp2p-request-response::cbor::Behaviour`) with `BTreeMap`/struct-field-order determinism for `signing_bytes`; pin `cbor4ii` as the single CBOR crate. Expose JSON as the agent contract and CBOR diagnostics separately.

4. **Amounts are `u64` smallest units, `u128` only for `sum==SUPPLY`.** `SUPPLY`, `DUST_THRESHOLD`, `MAX_AMOUNT` are consensus constants pinned in `genesis.json` + `meta` and enforced at the `validate_messages` gate before `Validation::Accept` (so dust never floods the mesh). `SUPPLY=1e15` at 6 decimals fits `u64` with headroom; JSON numbers stay safe. `timestamp` skew and rate-limit params remain local policy.

5. **Storage is `redb` with tables `balances: Table<&[u8],u64>`, `nonces: Table<&[u8],u64>`, `batches: Table<u64|Composite,&[u8]>`, `meta: Table<&str,&[u8]>`.** One `write_txn` per batch covers `balances+nonces+batches` atomically; `begin_read` concurrent for API/reputation. Invariant checked twice: fast `total==SUPPLY` per batch + periodic `u128` full-scan every N batches / on startup / on checkpoint. Wrap `Transfer` batches in `SignedBatch{ version:1, author, seq, transfers, vclock, sig }` where `seq` is the per-author ordering clock (see `ledger-replication.md` fork-choice).

6. **API is a shared `ai-bank-types` crate.** All ledger/API/sync types derive `Serialize+Deserialize+JsonSchema` once; `axum::Json<T>` handlers and `rmcp #[tool]` handlers call the same `service::validate_transfer / apply_batch_atomic / balance`. OpenAPI via `utoipa`/`aide` consumes the same `JsonSchema`. Tool descriptions state `amount` units and dust/nonce/sig domain for LLM accuracy.

7. **Versioning is explicit at two levels + protocol idents.** `META.schema_version: u32 = 1` (DB) and `SignedBatch.version: u8 = 1` (value) with additive-only `serde` evolution (`Option<T>` + `default`, never rename required fields). CBOR field order is frozen. Gossip topic `"/ai-bank/transfer/1.0.0"` and `"/ai-bank/sync/1.0.0"` carry the version; changing `Transfer` shape → bump `SignedBatch.version` + new topic + `Checkpoint` cutover. Migrations are forward-only `write_txn` transforms tested against `tests/fixtures/ledger-v1.redb`.

8. **Handoff contracts:** #9 (`ledger-replication`) imports `Transfer`/`SignedBatch`/`AccountId` types verbatim from here and replaces its `§4/§5` placeholders; #12 supplies `SUPPLY` + genesis allocation `BTreeMap<PeerId,u64>` that seeds `genesis.json`/`redb BALANCES`; #11 reads `balances+nonces+batches` via `begin_read` without coupling fork-choice to reputation (FAL-2: reputation display-only per `ledger-replication.md §3.1`).

---

## 11. Open Questions for Maintainer (Grilling / Decision Needed)

* **Supply & denomination (blocks #12 but constrained here):** Confirm `SUPPLY` integer and `decimals` (recommend `6`, i.e. `1 credit = 1_000_000 units`, `SUPPLY=1e15` for `1e9` credits). Must be pinned in `genesis.json` now (option A, compile-time constant `const SUPPLY: u64` + `blake3` pin) or per-network genesis param (option B, testnet flexibility). Option A simplifies `cargo test` invariants; option B requires `META.supply` read before validation.
* **Dust threshold — consensus or local?** Recommend **consensus** (genesis-pinned, hard-fork to change) for MVP so `gossipsub` validation is deterministic across peers (otherwise fork). Alternative is local-policy dust (each node independently drops small transfers but accepts histories that contain them) — simpler to tune but diverges `validate_messages` verdict. Pick one now; changing later is a social fork.
* **First nonce convention:** `1` (recommended, `0` sentinel for "no transfers yet") vs `0` (first transfer is `0`). Either is fine if documented; `1` matches BIP-32 / typical seq counters and keeps `genesis` distinct.
* **`BATCHES` key shape:** linearized `u64` global seq vs composite `(PeerIdBytes, u64)` per-author seq. Composite matches per-sender `nonce` semantics and needs no sequencer; linear is simpler for `SyncRequest{ since_seq }` range scans. Recommend **composite** unless a global sequencer is added — decision affects `redb` prefix-scan code in anti-entropy.
* **CBOR crate lock:** `cbor4ii 0.3` (align with `libp2p`) vs `ciborium 0.2` (simpler writer API). Both are correct — workspace must pick **one**. Default here is `cbor4ii` because it eliminates codec mismatch with `libp2p-request-response::cbor::Behaviour`.
* **Genesis encoding:** `genesis.json` (JSON, `jq`-friendly, hex sig) vs `genesis.cbor` (canonical, same bytes as ledger). Either can be shipped; recommend JSON file with canonical CBOR hash pin for human audit.

---

## Sources — Primary Only (Every Claim Traces Above)

* **`specs/peer-ids/peer-ids.md`** — `PeerId { multihash }`, identity multihash for Ed25519 ≤42B, `from_public_key`/`is_public_key`, legacy `12D3Koo` vs `bafz` [https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md](https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md)
* **`rust-libp2p/core/src/peer_id.rs` + `core/src/identity.rs` / `libp2p-identity`** — `Keypair::generate_ed25519`, `to_protobuf_encoding`/`from_protobuf_encoding`, `PeerId::to_bytes`/`from_bytes`/`to_base58`/`from_str`, `PublicKey::verify`, `Keypair::sign` [https://github.com/libp2p/rust-libp2p/blob/master/core/src/peer_id.rs](https://github.com/libp2p/rust-libp2p/blob/master/core/src/peer_id.rs) ; [`docs.rs libp2p-identity`](https://docs.rs/crate/libp2p-identity/latest) ; [`docs.rs libp2p_identity`](https://docs.rs/crate/libp2p-identity/latest)
* **`ed25519-dalek 2.1`** — `SigningKey`/`VerifyingKey`/`Signature::from_bytes`, RFC 8032 deterministic 64B sigs [https://crates.io/crates/ed25519-dalek](https://crates.io/crates/ed25519-dalek) ; [RFC 8032]
* **`serde.rs` + `serde_json` + `Serializer::is_human_readable`** — single `Serialize`/`Deserialize` for both JSON and CBOR, human-readable branch for base58/hex [https://serde.rs](https://serde.rs) ; [`docs.rs serde_json`](https://docs.rs/crate/serde_json/latest)
* **`schemars 1.1`** — `#[derive(JsonSchema)]`, JSON Schema 2020-12, `with = "String"` overrides, shared with `utoipa`/`aide` + `rmcp` tool schemas [https://docs.rs/crate/schemars/latest](https://docs.rs/crate/schemars/latest)
* **`axum 0.8`** — `Json<T>` extractor/responder, `Router`, `127.0.0.1` bind per ADR 0003 [https://docs.rs/crate/axum/latest](https://docs.rs/crate/axum/latest)
* **`rmcp 3.2`** — `#[tool]`/`#[tool_router]`, `transport-io`/`transport-streamable-http-server`, `CallToolResult`, JSON-RPC `-32602` [https://crates.io/crates/rmcp](https://crates.io/crates/rmcp)
* **`utoipa 5` / `aide`** — OpenAPI from `JsonSchema` [https://docs.rs/crate/utoipa/latest](https://docs.rs/crate/utoipa/latest)
* **RFC 8949 (CBOR)** — §4.2 Core Deterministic Encoding, `major type 2` byte string, preferred serialization [https://www.rfc-editor.org/rfc/rfc8949] ; RFC 8610 (CDDL) [https://www.rfc-editor.org/rfc/rfc8610]
* **`cbor4ii 0.3` / `ciborium 0.2` / `serde_cbor`** — CBOR crates; `libp2p-request-response::cbor` is `cbor4ii::serde` [https://docs.rs/crate/cbor4ii/latest] ; [`docs.rs libp2p::request_response::cbor`](https://docs.rs/libp2p/latest/libp2p/request_response/cbor/index.html) ; [`crates.io ciborium`](https://crates.io/crates/ciborium)
* **`redb 3.1.0/4.2.0`** — `Database::create`, `TableDefinition`, `write_txn.commit()`, MVCC one-writer-many-readers, **file format stable since 1.0 (2023-06-16)** [https://docs.rs/crate/redb/3.1.0](https://docs.rs/crate/redb/3.1.0) ; [`crates.io redb`](https://crates.io/crates/redb) ; benchmarks vs sled/lmdb
* **`sled 0.34.7`** — `Db::insert`/`apply_batch`/`transaction`, `flush`, **beta warning: "if reliability is your primary constraint, use SQLite. sled is beta"** + `on-disk format will change before 1.0, manual export migration` [https://docs.rs/crate/sled/0.34.7](https://docs.rs/crate/sled/0.34.7)
* **`blake3` / `hex` / `multihash 0.19` / `bs58 0.5`** — `blake3::hash`, `hex::encode/decode` [https://crates.io/crates/blake3] ; ADR 0001 impl crates
* **`ucan.xyz` / Macaroons** — UCAN `iss→aud` capability chain with `prf` proofs + attenuation, `did:key` Ed25519 DIDs; Macaroons caveat attenuation [https://ucan.xyz] ; [https://research.google/pubs/pub41892/]
* **`libp2p specs SignedEnvelope / PeerRecord` + `shared-registry.md §7.3`** — delegation-like self-certified `Seq + PeerId + addrs + sig` with `StoreInserts::FilterBoth` gate — precedent for future `Delegation` cert [Source: `shared-registry.md §7.3` + `specs SignedEnvelope`]
* **ADRs:** `0001-node-identity.md:1` (Ed25519 `PeerId`, petname, `b"/ai-bank/1/"`), `0002-communication-protocol.md:1` (swarm, Noise/TLS, `Signed`/`Strict`/`validate_messages`, topics `/ai-bank/transfer/1.0.0`), `0003-agent-interface.md:1` (localhost `axum` + `rmcp`, `Json<T>`, `schemars` reuse, `-32602`), `0004-safety-risk-levels.md:1` (FAL-2 fixed-supply, dust/nonce/fork-choice liveness, blast-radius), `0006-safety-evaluation-framework.md:1` (cargo harness, Sybil N=50, dust flood)
* **Research priors:** `ledger-replication.md` (gossipsub vs req-res vs kad, deterministic longest-valid-history, supply `u128` accumulator, `redb` schema sketch), `shared-registry.md` (Kademlia DHT — not ledger path, but `Signed PeerRecord` pattern)

---

*Prepared for wayfinder map #1. Next step: Draft ADR 0007 (data model) — `AccountId = PeerId`, `Transfer`/`SignedBatch` canonical types (`serde+schemars`, CBOR/JSON dual-codec via `is_human_readable`), `u64` amounts with `DUST`/`MAX` + `sum==SUPPLY` via `u128`, `redb` tables `balances/nonces/batches/meta` with atomic batch commit, `axum\Json`+`rmcp` sharing `ai-bank-types`, versioned persistence (`schema_version` + `SignedBatch.version`) — then close #10 and unblock #9/#11/#12.*

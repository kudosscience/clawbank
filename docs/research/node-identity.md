# Node Identity: How Nodes Prove Who They Are

**Wayfinder Research Ticket #2 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**  
**Branch:** `research/node-identity` | **Date:** 2026-09-02 | **Author:** Muse Spark (research subagent)  
**Status:** Research complete — decision-ready

---

## TL;DR for Decision-Maker

| Option | What it is | Verdict for AI Bank MVP |
|---|---|---|
| **A: Ed25519 keypair → PeerId (libp2p)** | 32-byte private key generated locally on first run; 32-byte public key hashed/multihashed into a `PeerId` (`12D3Koo...`). Identity = crypto, not a string. | **Recommended core identity.** Zero infrastructure, survives NAT/restarts, aligns with Rust P2P ecosystem. |
| **B: Human-readable names only (DNS-like)** | `"alice-node"`, registry maps name → address/key | **Reject as sole identity.** Requires central registry/CA to prevent squatting and spoofing — violates "no cloud bills" constraint. |
| **C: Hybrid — keypair + petname layer** | Cryptographic PeerId is canonical; local alias (`"alice-savings"`, `"mom"`) stored per-node in a petname address book. Optional shared registry stores `{PeerId → aliases[]}` as signed metadata. | **Recommended UX layer on top of A.** Gives humans readability without weakening security. |

**Bottom line:** Ship MVP with **(A) as source of truth + (C) petname mapping for display**. Do not invent a PKI server. Persistence is a local file (`~/.ai-bank/identity.key` protobuf). Verification is automatic during libp2p Noise/TLS handshake — no app-level CA needed.

---

## 1. How Nodes Generate and Prove Identity

### 1.1 Cryptographic keypair → PeerId (the libp2p model)

**Generation (first run):**

```rust
use libp2p_identity::Keypair;
let keypair = Keypair::generate_ed25519();          // OS CSPRNG, 32B secret + 32B public
let peer_id = keypair.public().to_peer_id();        // multihash( protobuf(public_key) )
println!("{}", peer_id.to_base58()); // e.g. 12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu
```

Storage: `keypair.to_protobuf_encoding()` → write to `~/.ai-bank/identity.key` (or OS keychain). Reload via `Keypair::from_protobuf_encoding(&bytes)`. [Source: `libp2p/rust-libp2p/core/src/identity.rs` — `Keypair::generate_ed25519`, `to_protobuf_encoding` / `from_protobuf_encoding`](https://github.com/libp2p/rust-libp2p/blob/320a1cde001381335f6c502f892259e701b13093/core/src/identity.rs); [Source: `libp2p-identity` docs — generation note: "Such identity keys can be randomly generated on every startup, but using already existing, fixed keys is usually required"](https://docs.rs/libp2p-identity/latest/libp2p_identity/)

**What PeerId actually is:**

- Deterministic `multihash( protobuf(public_key) )`. For Ed25519 (≤42 bytes serialized), uses `identity` multihash (no hash, just wrapped bytes); for RSA (>42 bytes) uses `sha2-256`. [Source: `libp2p/specs/peer-ids/peer-ids.md` § Peer Ids](https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md); [Source: `libp2p/rust-libp2p/core/src/peer_id.rs` — `PeerId { multihash: Multihash }`, `from_public_key`](https://github.com/libp2p/rust-libp2p/blob/c0b379b908a2f1f622cd205c6890a520bc8f5949/core/src/peer_id.rs)
- Two text encodings: legacy `base58btc` (`12D3Koo...`, `Qm...`) and new CIDv1+`libp2p-key` multicodec in base32 (`bafz...`). Implementations MUST parse both. [Source: `peer-ids.md`](https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md)
- `PeerId::is_public_key(&pubkey) -> Option<bool>` lets any node verify a claimed public key matches a PeerId without a directory lookup. [Source: `peer_id.rs`](https://github.com/libp2p/rust-libp2p/blob/c0b379b908a2f1f622cd205c6890a520bc8f5949/core/src/peer_id.rs)

**Proving identity (signing):**

- Node holds private key locally, never transmits it. To prove ownership it signs a challenge/message:

```rust
use libp2p_identity::Keypair;
let sig = keypair.sign(b"ai-bank:nonce:42").unwrap();
assert!(keypair.public().verify(b"ai-bank:nonce:42", &sig));
```

- Under the hood Ed25519 (RFC 8032) — deterministic, 64-byte signatures, ~microsecond verification. [Source: `libp2p-identity` trait `SigningError`, `PublicKey::verify`](https://docs.rs/libp2p-identity/latest/libp2p_identity/); Ed25519 choice mandated: "Implementations MUST support Ed25519" per spec.

**Strengths:** No central registry, keys generated offline, identity stable across IP changes / restarts, cryptographic binding to ledger transactions (sign transfers with same key or derived account key).

**Weaknesses:** PeerIds are not human-memorable; lose the key file and you lose the identity (needs backup/seed phrase consideration); public key rotation = new PeerId unless you add an indirection layer.

### 1.2 Human-readable names alone

- Examples: DNS (`alice.ai-bank.local`), ENS-style, or a shared registry table `{ "alice": PeerId }`.
- Requires a **naming authority** to prevent collisions (`alice` claimed by two nodes) and squatting.
- Zooko's Triangle applies: a name system can have at most two of {decentralized, globally unique, human-meaningful}. Pure names sacrifice either decentralization (central registry) or uniqueness (collisions). [Source: Spritely Petnames paper — Zooko's Triangle discussion](https://files.spritely.institute/papers/petnames.pdf)
- For "no cloud bills / runs on user's machines", a centrally-hosted name registry is a direct violation — someone pays for the server and becomes a trust bottleneck.

**When it makes sense:** As a *curated community directory* (edge names — e.g., `dns:paypal.com` imported as a hub) layered *on top of* crypto IDs, not as the ID itself.

### 1.3 Hybrid — keypair for crypto, name for humans (recommended)

Two widely-adopted patterns:

**a) Local petname table (no global coordination):**
Each node keeps `HashMap<PeerId, String>` like a smartphone contacts list. `PeerId 12D3KooWFoo… → "mom"`. Presentation layer shows petname; verification layer always uses PeerId. This is exactly the model proposed in the Spritely Petnames paper: petname (local) + edge name (introduced via a trusted introducer) + self-proposed name. [Source: Petnames paper §§ 2.1–2.2](https://files.spritely.institute/papers/petnames.pdf)

**b) Shared signed records (OPTIONAL, later):**
A replicated registry (Kademlia DHT, gossipsub, or ledger) stores `{ PeerId, public_key, aliases[], seq, signature }` as a `SignedEnvelope` / `PeerRecord`. Anyone can verify the alias claim was signed by the PeerId's private key, but aliases are *hints*, not authority. libp2p's `CertifiedAddrBook` + `Signed Peer Records` are designed for exactly this: self-certified addresses without a CA. [Source: libp2p peerstore — `CertifiedAddrBook::ConsumePeerRecord`](https://pkg.go.dev/github.com/libp2p/go-libp2p/core/peerstore#CertifiedAddrBook); [Source: libp2p docs — Peer Store as address book](https://libp2p.io/docs/peers/)

| Aspect | Crypto-only (A) | Hybrid (A+C) |
|---|---|---|
| Global uniqueness | Yes (collision ~2^-256) | Yes (PeerId remains key) |
| Human meaning | No | Yes (local petnames + shared hints) |
| Needs server? | No | No for local table; optional DHT for shared hints |
| Spoofing risk | None if signature checked | Phishing if UI shows unverified alias — mitigate by showing `alias (PeerId abbr)` and verifying signatures |

---

## 2. How Identity Is Shared and Verified During P2P Communication

### 2.1 At connection establishment (handshake — automatic)

libp2p does **not** rely on a CA. Verification happens inside the encrypted handshake:

- **Noise (XX pattern, X25519):** Each side generates an ephemeral X25519 DH key, but links it to the long-term identity key by signing the DH public key with the Ed25519 identity key (`Protocol::sign` / `verify`). Remote proves possession of identity private key without exposing it. Output of the upgrade is `(PeerId, NoiseOutput)`. [Source: `libp2p-noise` — `NoiseAuthenticated`, `Protocol::linked/verify/sign`](https://docs.rs/libp2p/latest/libp2p/noise/index.html); Note: "Only the XX handshake pattern is currently guaranteed to provide interoperability"]
- **TLS 1.3 variant:** Identity public key is embedded in a self-signed X.509 extension (`OID 1.3.6.1.4.1.53594.1.1` — libp2p Public Key Extension). Peer signs `libp2p-tls-handshake:<SPKI>` with identity key; verifier derives PeerId from the extension and checks signature + that it matches the expected PeerId. No external cert chain. [Source: `libp2p/specs/tls/tls.md` — libp2p Public Key Extension, Peer Authentication](https://github.com/Nashatyrev/libp2p-specs/blob/master/tls/tls.md)

Both paths end with: caller supplies expected `PeerId` (if dialing a known peer) or accepts the derived one; `InboundSecurityUpgrade` / `OutboundSecurityUpgrade` aborts before application data flows if verification fails. [Source: PR #4864 — verify expected PeerId as part of security handshake](https://github.com/libp2p/rust-libp2p/pull/4864)

**What this means for AI Bank:** You do not need to write verification code for the transport layer — `SwarmBuilder::with_noise` / `.with_tls` (or equivalent `Transport::upgrade(...).authenticate(noise::Config::new(&keypair))`) handles it. App code only needs to check `peer_id.is_public_key(&advertised_key)` when learning peers out-of-band.

### 2.2 During application messaging

- Every ledger transaction / registry update should be a signed envelope: `payload || Ed25519_signature || public_key`. Recipient calls `PublicKey::verify(payload, &sig)` and confirms `PeerId::from_public_key(&pubkey) == claimed_sender`.
- Use domain separation: sign `b"/ai-bank/1/transfer:" || payload_bytes` to prevent cross-protocol replay. (`minip2p-identity` pattern recommends `b"/minip2p/1" || payload`.)
- For gossipsub or request-response, signature is checked at handler entry; invalid → drop + peer-score penalty.

### 2.3 Discovery (how a node learns another's PeerId + address)

1. **Bootstrap:** Hardcoded or user-supplied `Multiaddr` with embedded PeerId, e.g. `/ip4/1.2.3.4/tcp/4001/p2p/12D3KooWFoo…`.
2. **Identify protocol:** After handshake, `identify` exchange pushes `{ public_key, listen_addrs, observed_addr, protocols }`. Receiver stores in `PeerStore` (address book). [Source: libp2p docs — Peer Store](https://libp2p.io/docs/peers/); [Source: `peerstore` Go proposal — address book is the surviving component](https://github.com/libp2p/go-libp2p/issues/2355)
3. **DHT / mDNS / registry:** Later phases, but all converge on the same `PeerId → {addrs, public_key}` mapping, always self-verifiable.

---

## 3. Rust Ecosystem — What Crates Exist

### 3.1 Primary (use directly)

| Crate | Role | Version note | Feature flags |
|---|---|---|---|
| **`libp2p-identity`** (formerly `libp2p-core::identity`) | `Keypair`, `PublicKey`, `PeerId`, sign/verify, protobuf encode | `0.2.13` current; split from `libp2p-core` in 2023 | `ed25519` (default via `ed25519-dalek 3.x`), `secp256k1`, `ecdsa`, `rsa` (non-wasm), `peerid`, `rand` |
| **`ed25519-dalek`** | Low-level Ed25519 if you need keys outside libp2p wrapper (e.g., signing ledger txs with same key) | `2.1.0` current; `SigningKey`/`VerifyingKey`/`Signature` | `rand_core`, `pkcs8`, `pem`, `batch`, `zeroize` (on), `digest` for Ed25519ph |
| **`multihash`** | Hash abstraction for PeerId derivation | `0.19.x` | — |
| **`bs58`** | Base58btc encoding for `PeerId::to_base58()` | `0.5.x` | — |

Sources: [`libp2p-identity` on crates.io](https://crates.io/crates/libp2p-identity), [`libp2p-identity` docs.rs — "Data structures and algorithms for identifying peers in libp2p"](https://docs.rs/libp2p-identity/latest/libp2p_identity/), [`ed25519-dalek` on crates.io / docs.rs — "Fast and efficient ed25519 … in pure Rust"](https://crates.io/crates/ed25519-dalek/2.1.0), [`ed25519-dalek` docs — `SigningKey::generate`, `Signer`/`Verifier` traits](https://docs.rs/ed25519-dalek/latest/ed25519_dalek/)

**Important incompatibility warning** (from libp2p docs): "keys of external ed25519 or secp256k1 crates cannot be directly converted into libp2p network identities … Instead, loading fixed keys must use the standard, thus more portable binary representation (e.g. ed25519 binary format 32B)". Concretely: do not try `ed25519_dalek::SigningKey → libp2p_identity::Keypair` via direct struct cast; round-trip through `to_bytes()` / `from_bytes()` or protobuf. [Source: `libp2p-identity` module docs](https://docs.rs/libp2p-identity/latest/libp2p_identity/)

### 3.2 Supporting / transport

| Crate | Role | Note |
|---|---|---|
| `libp2p-noise`, `libp2p-tls` | Authenticated encryption upgrades (Section 2) | Pick one (Noise is simpler for MVP; TLS gives WebTransport compat) |
| `libp2p` (umbrella) | `Swarm`, `Transport`, `Multiaddr` | Re-exports `libp2p-identity` |
| `minip2p-identity` | `no_std`-friendly alternative libp2p identity (same protobuf + multihash logic) | `0.2.0` (2026-07-27), deterministic encoding, `Ed25519Keypair::generate()` |
| `k256`, `p256`, `ring` | Secp256k1/ECDSA/RSA if you ever need non-Ed25519 — avoid for MVP (adds size + complexity) | Enabled via `libp2p-identity` features |

### 3.3 Do-not-need (but often confused)

| Crate / system | Why not for MVP |
|---|---|
| `did:key` / `ssi` / `iota-identity` / `archon` / `kery` | Full DID/VC stacks with ledgers, registries, IPFS — heavy for a 3-node MVP; adopt later if agents need W3C DIDs. |
| `iroh` | Opinionated QUIC+relay stack; great but couples identity to iroh's ticket system. Prefer vanilla libp2p so ledger/registry remain agnostic. |
| Central PKI (`rcgen`, `webpki`, ACME) | Requires a CA domain and renewal — violates "no cloud bills" and adds an operator. |

---

## 4. What "No Cloud Bills, Runs on User's Machines" Implies

**No central CA, no PKI server, no hosted registry.**

- **Generation is local:** `Keypair::generate_ed25519()` uses OS CSPRNG — no network call, no registration ceremony. Cost is a single 64-byte file on disk. Implication: onboarding is `cargo run` → identity exists.
- **Verification is local:** `PublicKey::verify(msg, sig)` and `PeerId::is_public_key(&key)` need only the two byte strings. No OCSP/CRL lookup.
- **Persistence is the operator's job:** Store `to_protobuf_encoding()` under `~/.ai-bank/identity.key` with `0o600` permissions; optionally encrypt with a passphrase or OS keychain. Back up the 32-byte seed (or 64-byte protobuf) like a wallet seed. If the file is lost, the identity is lost — document this as a known MVP limitation and offer `ai-bank export-key` / `import-key`.
- **Key rotation is explicit:** Rotating = new PeerId. If continuity is needed, publish a signed rotation statement `{old PeerId → new PeerId, sig_old}` to the shared registry/ledger and have peers honor it. Don't hide this behind a CA.
- **No privileged introducer:** The ledger/registry itself must not become a de-facto CA. Bootstrap peers are just well-known `Multiaddr`s; any node can be a bootstrap. Reputation (from #1's "transaction history") should key off `PeerId`, not alias.
- **NAT traversal without cloud relay is limited:** Without a relay, two nodes behind symmetric NATs cannot directly dial. Pure P2P still needs either UPnP, hole-punching (libp2p DCUtR), or an opt-in community relay. The identity layer itself does not solve this, but using libp2p PeerIds keeps the door open to add a relay later without re-issuing identities.

**What you give up by rejecting cloud identity:**

- Revocation lists become "last-write-wins" signed records, not instant.
- No human-enforced moderation of names (mitigate with petnames + `alias (PeerId short)` display).
- Discovery is slower (DHT/mDNS vs. a single Postgres). Acceptable for MVP (<100 nodes).

---

## 5. Recommendation for AI Bank MVP

### Decision: Ed25519 keypair + PeerId as canonical identity, petname overlay for humans

**Steps to implement:**

1. **Crate:** Depend on `libp2p-identity = { version = "0.2", features = ["ed25519","peerid","rand"] }` (or umbrella `libp2p` which re-exports it). Optionally `ed25519-dalek` only if you need raw signing outside libp2p.
2. **Generation & storage:**
   - On first `ai-bank init`, `Keypair::generate_ed25519()` → `to_protobuf_encoding()` → `~/.ai-bank/identity.key` (create dir `0o700`, file `0o600`).
   - Also print `PeerId::to_base58()` and `PeerId::to_string()` (CID form future-proof) + QR for agent config.
   - Provide `ai-bank identity export --seed-phrase` (BIP-39 wrapper over 32B seed) as backup if desired; out-of-scope for MVP but stub the CLI.
3. **Transport:**
   - Use `libp2p-noise` (XX) for MVP — single `NoiseConfig::new(&keypair)` line, no cert management.
   - `Swarm::dial(peer_id, multiaddr)` already verifies; no extra app code.
4. **Petname layer:**
   - Local JSON `~/.ai-bank/peers.json`: `{ "12D3KooWFoo…": { "alias": "alice-savings", "added": "2026-09-02", "publicKey": "CAESIC…" } }`.
   - CLI: `ai-bank peers add <peerId> --alias alice` / `ai-bank peers list` shows `alias  PeerId(abbr)  last_seen`.
   - UI for agents: local HTTP API returns both — `{ "peerId": "12D3KooWFoo…", "alias": "alice-savings" }` — so LLMs can use either but ledger writes use PeerId.
5. **Signing app messages:**
   - Define `domain_separator = b"/ai-bank/1/"`.
   - For transfers: `sig = keypair.sign(concat(domain_separator, cbor(transaction)))`; verify in ledger replication handler.
6. **Shared alias hints (defer past MVP):** When a registry/DHT exists, publish a `PeerRecord` signed by the node's key. Treat as hint, not truth.

**Alternatives considered and rejected:**

- RSA/Secp256k1/ECDSA keys: larger keys/certs, slower, not needed; spec says MUST support Ed25519, MAY support others. Stay Ed25519-only for MVP.
- W3C DID (`did:key`/`did:web`): adds JSON-LD, resolution, ledger anchoring — premature.
- Username/password or API token: replayable, not self-sovereign, needs a server to store hashes.

### Open questions to resolve with other tickets

- **#4 (Communication protocol):** Confirm Noise vs. TLS choice; identity decision works with either.
- **Ledger binding:** Does the ledger account ID equal the PeerId, or is account a separate Ed25519 key derived from the node key? (PeerId = node operator; account = agent wallet — recommendation: keep separate, with a delegation signature `node_key signs "account X belongs to PeerId Y"`.)
- **Persistence encryption:** Plaintext protobuf is simplest but users may expect passphrase-encrypted storage — decide before first release.

---

## Appendix: Primary Sources

- `libp2p/specs/peer-ids/peer-ids.md` — key types, protobuf encoding, PeerId derivation (≤42 bytes identity vs. >42 bytes sha2-256), text encodings. [Link](https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md)
- `libp2p/rust-libp2p/core/src/identity.rs` — `Keypair::generate_ed25519()`, `to_protobuf_encoding` / `from_protobuf_encoding`, `PublicKey::verify`, warning about external crate conversion. [Link](https://github.com/libp2p/rust-libp2p/blob/320a1cde001381335f6c502f892259e701b13093/core/src/identity.rs)
- `libp2p/rust-libp2p/core/src/peer_id.rs` — `PeerId` as `Multihash`, `from_public_key`, `is_public_key`, `to_base58`, `random()`. [Link](https://github.com/libp2p/rust-libp2p/blob/c0b379b908a2f1f622cd205c6890a520bc8f5949/core/src/peer_id.rs)
- `libp2p-identity` docs — split crate, features, MUST/SHOULD key support note. [Link](https://docs.rs/libp2p-identity/latest/libp2p_identity/) / [crates.io](https://crates.io/crates/libp2p-identity)
- `ed25519-dalek` docs/crate — `SigningKey::generate`, `Signer`/`Verifier`, `verify_strict` weak-key note. [Link](https://docs.rs/ed25519-dalek/latest/ed25519_dalek/) / [Link](https://crates.io/crates/ed25519-dalek/2.1.0)
- `libp2p/specs/tls/tls.md` — libp2p Public Key Extension OID 1.3.6.1.4.1.53594.1.1, `libp2p-tls-handshake:` prefix, X.509 self-signed carrying identity key. [Link](https://github.com/Nashatyrev/libp2p-specs/blob/master/tls/tls.md)
- `libp2p-noise` docs — `NoiseAuthenticated`, `XX` pattern, `Protocol::sign/verify/linked`, X25519 static DH bound to identity. [Link](https://docs.rs/libp2p/latest/libp2p/noise/index.html)
- PR #4864 — verifying expected PeerId during handshake via `InboundSecurityUpgrade`. [Link](https://github.com/libp2p/rust-libp2p/pull/4864)
- Petnames paper — Zooko's Triangle, petname/edge/self-proposed names, smartphone contacts & browser bookmark analogy. [Link](https://files.spritely.institute/papers/petnames.pdf)
- `go-libp2p` peerstore — `AddrBook` vs. `CertifiedAddrBook` + `ConsumePeerRecord` for self-certified signed peer records without CA. [Link](https://pkg.go.dev/github.com/libp2p/go-libp2p/core/peerstore) / issue #2355
- `minip2p-identity` — no_std Ed25519 + PeerId derivation, domain-separated signing note. [Link](https://docs.rs/crate/minip2p-identity/latest)

---

*Next step: Decision-maker reviews §5 and opens an ADR (`docs/adr/0001-node-identity.md`) locking in Ed25519/PeerId + petname. Ticket #2 can then be closed with a pointer to this file.*

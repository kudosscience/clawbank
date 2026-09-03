# Ledger Replication & Supply Invariant: Fork-Choice Without a Blockchain

**Wayfinder Research Ticket #9 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**
**Branch:** `research/ledger-replication` | **Date:** 2026-09-03 | **Author:** Muse Spark (research subagent)
**Status:** Research complete — decision-ready
**Depends on:** #10 (Account & transaction data model) — record shape, CBOR/JSON, amount/nonce types deferred to #10; this doc researches the transport/storage layer in parallel
**Depends on:** ADR 0001 (Ed25519 `PeerId`), ADR 0002 (libp2p swarm), ADR 0004 (FAL-2 fixed-supply), ADR 0006 (evaluation harness)

---

## TL;DR for Decision-Maker

| Option | What it is | Verdict for AI Bank MVP |
|---|---|---|
| **A: `gossipsub` mesh (`libp2p-gossipsub`, topic `/ai-bank/transfer/1.0.0`, `MessageAuthenticity::Signed`, `ValidationMode::Strict`, `validate_messages=true`, peer scoring)** | Bounded-degree mesh (D=6, D_low=4, D_high=12) with heartbeat gossip, IHAVE/IWANT cache repair, and built-in peer scoring (P₁–P₇). Application validates every message before `Validation::Accept` propagation. | **Recommended as primary ledger replication transport.** Best fit for "every transfer reaches every online peer" broadcast without a blockchain. Decouples dissemination from ordering; ordering is handled by deterministic fork-choice + supply-invariant validation at the application layer. |
| **B: `request-response` (`libp2p-request-response` `cbor::Behaviour` / `json::Behaviour`)** | One substream per request (CBOR over Yamux/QUIC). `send_request(peer, TransferBatch)` → `Event::OutboundFailure/InboundRequest/ResponseSent`. Requires peer discovery (kad or gossipsub) to know *who* to dial. | **Required as complement for catch-up/sync, not replacement for replication.** Use for anti-entropy: "give me batches since `seq=n`" after partition or offline. Do not use for fan-out — O(n) dials per transfer, no amplification control. |
| **C: Kademlia provider records (`libp2p-kad` `start_providing`/`get_providers`)** | Pull-model pointer: DHT stores `ProviderRecord{key, provider: PeerId, expires}` at `k=20` closest nodes to `sha256(key)`. TTL 48h/republish 22h. Consumer must dial provider to fetch actual bytes via B. | **Reject as ledger propagation.** DHT is for peer routing per ADR 0002/registry (#8), not for ledger liveness. `get_providers` has no push notification; replicas expire; no mesh scoring. Useful only for "which peers *claim* to have shard X" discovery, not for supply-critical transfers. |
| **D: Storage choice `redb` vs `sled` for local persistence** | `redb` (pure Rust, copy-on-write B+-trees, ACID MVCC, stable file format since 1.0) vs `sled` (lock-free Bw-tree, log-structured, 0.34.7 stable since 2021, beta warnings, unstable format pre-1.0 with export-only migration). | **Recommended: `redb 3.1+ (or 4.x)` for ledger log.** ACID + stable format + maintained (actively developed, 1.1M downloads/mo). Reserve `sled` only if lock-free concurrent-write throughput is measured to dominate and migration cost of pre-1.0 format is accepted. See §7. |

**Bottom line:** Ship ledger on **`gossipsub` (A) for push propagation + `request-response CBOR` (B) for pull catch-up + `redb` (D) for local durable log + deterministic fork-choice (longest-valid-history with supply-invariant filter) at the application layer**. Keep DHT provider records (C) out of the critical path. Wire `identify → kad` per #8 only for peer routing/anti-entropy dialing, not for transfer broadcast. All messages are CBOR/JSON transfer batches signed under domain `b"/ai-bank/1/"` (same domain as ADR 0001 identity sig) and verified against `PeerId` via `libp2p-identity::PeerId` / `ed25519-dalek`.

---

## 1. What Ledger Replication Must Achieve at FAL-2

FAL-2 constraints from ADR 0004/0006 shape the design:

- **Fixed-supply virtual credits, genesis mint only** — every node must independently verify `sum(balances) == SUPPLY` after applying any batch; no inflation op may exist post-genesis.
- **No blockchain / no global consensus** — tolerate partitions, offline peers, and Sybil (FAL-2 harness tests N=50 Sybil) without PoW/PoS or a leader. "Consensus" is local deterministic reconciliation + social-fork checkpoint, not BFT agreement.
- **Contained blast radius** — worst case at FAL-2 is recoverable ledger corruption / reputation collapse / DoS via gossip, not irreversible financial loss; so fork-choice may discard a branch and social-fork may re-anchor genesis rather than requiring Byzantine fault tolerance with 2/3 quorum.
- **No cloud bills / runs on user hardware** — replication must work on intermittent laptops behind NAT (relay + DCUtR per ADR 0002), with local verification (`cargo test`) as the evaluation harness.

Therefore the ledger is an **eventually consistent replicated log of signed transfers**, validated locally, reconciled deterministically.

---

## 2. Replication Transports: Gossipsub vs Request-Response vs Kad Providers

### 2.1 Comparison Table

| Dimension | `gossipsub` mesh (A) | `request-response` CBOR (B) | Kademlia provider records (C) |
|---|---|---|---|
| **Pattern** | Push broadcast: publish once → mesh forwards to D peers + gossip to non-mesh via IHAVE/IWANT [Source: `specs/pubsub/gossipsub/gossipsub-v1.0` — mesh, fanout, control messages](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md) | Unicast RPC: one new substream per request over Yamux/QUIC [Source: `docs.rs/libp2p::request_response::Behaviour::send_request` — new substream per request, dial if not connected](https://libp2p.github.io/rust-libp2p/libp2p/request_response/struct.Behaviour.html) | Pull pointer: publish provider record to k closest nodes via iterative lookup; discover via `GET_PROVIDERS` [Source: `libp2p.io/guides/dht` + `docs.rs libp2p-kad::Behaviour::start_providing/get_providers`](https://libp2p.io/guides/dht/) |
| **Amplification** | Bounded by D=6 target, gossip factor 0.25, heartbeat 1s; duplicates suppressed by `MessageCache` + `DuplicateCache` [Source: `docs.rs libp2p-gossipsub Config` — `mesh_n`, `history_length`, `duplicate_cache_time`](https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Config.html) | 1× per target peer (caller pays N dials); no gossip amplification | O(log n) DHT hops per record; `k=20` copies |
| **Topology awareness** | Mesh overlay self-heals via GRAFT/PRUNE + opportunistic grafting; `score<0` peers pruned [Source: `specs gossipsub-v1.1` — Peer Scoring, Thresholds, Heartbeat Maintenance](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md) | None — needs external discovery (kad `get_closest_peers` or explicit addrs) [Source: `request_response::Behaviour::send_request` note: "must be embedded in another NetworkBehaviour that provides peer discovery, or addresses managed via Swarm::add_peer_address"](https://libp2p.github.io/rust-libp2p/libp2p/request_response/struct.Behaviour.html) | DHT routing table (k-buckets, K=20, α=3) [Source: `shared-registry.md §4` + `libp2p.io/docs/kademlia-dht`](https://libp2p.io/docs/kademlia-dht/) |
| **Ordering guarantee** | None — eventually every subscriber sees every message (if mesh connected), but arrival order is non-deterministic | Point-to-point FIFO per substream, not global | None — `get_providers` returns provider set + closer peers, not ordered log |
| **Spam/DoS control** | Built-in: `PeerScoreParams` P₁–P₇ (time-in-mesh, first deliveries, mesh delivery rate/failures, invalid messages, app score, IP colocation, behavioural penalties) with decay, plus `validate_messages=true` gate [Source: `specs gossipsub-v1.1 Peer Scoring` — P₁..P₇ score function, DecayToZero](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md) + [`libp2p-gossipsub 0.49 ConfigBuilder` scoring fields](https://docs.rs/libp2p-gossipsub/0.49.0/libp2p_gossipsub/config/struct.ConfigBuilder.html) | App-controlled: reject per `Event::InboundRequest` before `send_response`; no mesh scoring — caller must rate-limit via service layer | `StoreInserts::FilterBoth` on inbound `PutRecord/AddProvider` [Source: `docs.rs libp2p-kad::StoreInserts`](https://docs.rs/libp2p/latest/libp2p/kad/enum.StoreInserts.html) |
| **Offline / partition recovery** | Missed messages recovered via IHAVE/IWANT only within `mcache` window (`history_length=5, history_gossip=3` heartbeats); long partitions lose messages if cache expired | Catch-up RPC fetches arbitrary history — ideal for partition healing | Long-lived records survive if ≥1 of k replicas stays online; but expiry 36h/48h loses un-republished data [Source: `shared-registry.md §7.1` — `record_ttl 36h`, `provider_record_ttl 48h`](https://github.com/libp2p/specs/blob/master/kad-dht/README.md) |
| **Crate surface** | `libp2p-gossipsub 0.49.x` (`Behaviour`, `Config`, `ConfigBuilder`, `MessageAuthenticity`, `ValidationMode`, `TopicHash`, `IdentTopic`, `Event::Message/Subscribed/Graft/Prune`) [Source: `crates.io/libp2p-gossipsub 0.49.0`, `docs.rs libp2p_gossipsub`](https://docs.rs/crate/libp2p-gossipsub/0.49.0) | `libp2p-request-response 0.29.x` (`cbor::Behaviour<Req,Res>`, `json::Behaviour`, `Codec`, `Config`, `ProtocolSupport`, `Event`, `OutboundRequestId`) [Source: `crates.io/libp2p-request-response`, `docs.rs libp2p::request_response::cbor`](https://docs.rs/crate/libp2p-request-response/latest) | `libp2p-kad 0.48.x` (`Behaviour::start_providing`, `get_providers`, `ProviderRecord`) [Source: `crates.io/libp2p-kad 0.48.0`](https://crates.io/crates/libp2p-kad) |
| **When to use for ledger** | **Primary:** `publish(IdentTopic("/ai-bank/transfer/1.0.0"), cbor_batch)` + `subscribe` + `validate_messages` gate → local `apply_batch` | **Required secondary:** `SyncRequest{ since: Seq, limit }` → `SyncResponse{ batches }` for antientropy after reconnect | **Not for ledger:** only for optional "who has ledger shard / who offers snapshot" index if needed later; never for supply-critical broadcast |

### 2.2 Why Gossipsub Wins for Broadcast (A)

- **Amplification-tuned broadcast:** Mesh degree D=6 (default) keeps fan-out predictable on laptops; heartbeat (1s default) gossips `IHAVE` to non-mesh peers for eventual delivery without flooding [Source: `docs.rs Config::mesh_n/mesh_n_low/mesh_n_high/heartbeat_interval`](https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Config.html) and `specs/gossipsub-v1.0 — mesh, gossip, lazy push`](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md).
- **Validation-before-forward:** With `ConfigBuilder::validate_messages(true)`, the node must call `report_message_validation_result(&msg_id, Validation::Accept/Reject/Ignore)` before the message is forwarded; application checks signature, supply-invariant, nonce, dust before `Accept` [Source: `docs.rs Config::validate_messages` — "prevents automatic forwarding ... user must manually call report_message_validation_result"](https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Config.html) ; [`Behaviour::validate_messages` doc + `MessageAcceptance`](https://libp2p.github.io/rust-libp2p/libp2p/gossipsub/struct.Behaviour.html). Default `false` auto-forwards — **must be set true for ledger**.
- **Peer scoring is free Sybil/DoS damping:** P₄ (invalid messages) with negative weight ejects peers that spam invalid batches; P₃/P₃b (mesh delivery failures) ejects silent peers; scores decay to zero on `DecayToZero` threshold and are retained across disconnects to prevent score-reset whitewash [Source: `specs gossipsub-v1.1 — Score Function, P₄, Parameter Decay, decay_to_zero`](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md). Map P₅ `ApplicationSpecific` to reputation-weighted penalty (see §3.4).

### 2.3 Why Request-Response Is the Right Complement (B)

- **Generic codec model:** `Codec` trait defines `Request/Response` types + `Protocol`; `cbor::Behaviour` uses `cbor4ii::serde` (or `serde_cbor`) for ledger batches; `json::Behaviour` available for debug [Source: `docs.rs libp2p::request_response::cbor — Behaviour alias "using cbor4ii::serde"`](https://docs.rs/libp2p/latest/libp2p/request_response/cbor/index.html) ; [`libp2p-request-response crate — cbor/json behaviours, Protocol Families as sum types`](https://lib.rs/crates/libp2p-request-response).
- **Explicit dial or address-aware send:** `send_request(peer, req)` auto-dials if not connected but requires discovery; `send_request_with_addresses(peer, req, addrs)` supplies addrs atomically whenkad/identify hasn't yet populated the routing table [Source: `docs.rs Behaviour::send_request/send_request_with_addresses` signatures + discovery note](https://libp2p.github.io/rust-libp2p/libp2p/request_response/struct.Behaviour.html).
- **Config controls:** `Config::with_request_timeout(Duration)`, `Config::with_max_concurrent_streams`, inbound timeout — tune so slow anti-entropy doesn't stall gossipsub heartbeat on the same swarm [Source: `docs.rs libp2p::request_response::Config`].
- **Anti-entropy pattern (recommended):**
  ```rust
  // request-response protocol family
  #[derive(Serialize, Deserialize)]
  enum LedgerProtocol {}
  #[derive(Serialize, Deserialize)] struct SyncRequest { since_seq: u64, limit: u16 }
  #[derive(Serialize, Deserialize)] struct SyncResponse { batches: Vec<SignedBatch> }

  // on reconnect / periodic tick:
  let id = behaviour.request_response.send_request(&peer, SyncRequest{ since_seq: local_tip, limit: 64 });
  // handle Event::Message{ message: SyncResponse{..} } → validate + apply in fork-choice order
  ```
  Keep request-response **off the critical path**: broadcast via gossipsub is optimistic; sync RPC repairs gaps. This mirrors `ipfs-kad` + `gossipsub` layering in `rust-libp2p` examples [Source: `examples/chat` uses gossipsub + identify + kad together; `examples/ipfs-kad` shows kad+identify wiring](https://github.com/libp2p/rust-libp2p/tree/master/examples).

### 2.4 Why Kad Providers Are Wrong for Ledger Push (C)

- **No push, only poll:** `start_providing(key)` publishes a pointer, not bytes; peers learn nothing until they actively `get_providers(key)` and then dial the returned `PeerId`s to fetch via request-response. No `Event` tells you a new provider appeared [Source: `shared-registry.md §1` + `libp2p.io DHT — content routing via GET_PROVIDERS`](https://libp2p.io/guides/dht/) and `Discuss libp2p/1393 — provider is someone who claims to have the thing, get_providers returns list to contact`](https://discuss.libp2p.io/t/what-is-providers-of-a-value-to-the-given-key/1393).
- **Expiry loses ledger tail:** Provider records expire after 48h (default), value records after 36h; a transfer stored only as a DHT record vanishes unless re-published every 22h/24h [Source: `shared-registry.md §7.1` table from `behaviour.rs Config defaults`](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html).

**Recommendation for AI Bank:** Use **A for fan-out, B for repair, C only for optional shard-pointer registry** (e.g., `/ai-bank/snapshot/<checkpoint-seq>` → provider set), never for individual transfers.

---

## 3. Fork-Choice Without a Blockchain: Deterministic Rules

"Fork" here means two valid-but-divergent local histories after a partition or concurrent publish, not a blockchain reorg. No PoW/PoS, no leader, no total-order broadcast. Goal: every correct node converges to the **same single history** given the same set of observed batches, without coordination, and with supply invariant enforced.

### 3.1 Option Space

| Rule | Description | Determinism | Clock assumption | Supply safe? | Verdict |
|---|---|---|---|---|---|
| **Last-Write-Wins (LWW) per key** | Each account map entry keeps `max(timestamp)` writer; concurrent writes to same account pick highest wall-clock time, tiebreak by `PeerId` bytes [Source: `crdt-study — LWW assumes timestamps unique, totally ordered, monotonic; tolerates skew unless vector clock / NTP`](https://github.com/agravier/crdt-study) | Deterministic if timestamps unique; non-causal — concurrent reply may sort before causal predecessor | Requires NTP or HLC | No — LWW silently drops the losing write, breaks `sum==SUPPLY` if loser was part of supply accounting | **Reject for ledger state** |
| **Vector clocks (causal tracking)** | Each transfer carries `VectorClock{ peer: PeerId, counter: u64 }` per-sender counter; `V_a < V_b` iff ∀i V_a[i] ≤ V_b[i] ∧ ∃i < ; `V_a || V_b` (concurrent) iff neither dominates [Source: `LowLevelDesignMastery — Vector clock comparison V1<V2 vs concurrent V1‖V2`](https://www.lowleveldesignmastery.com/hld-concepts/consistency/05-conflict-resolution/) + `GeeksforGeeks vector clocks — establish sequence without world clock`](https://www.geeksforgeeks.org/computer-networks/vector-clocks-in-distributed-systems/) | Partial order — detects concurrency but needs a tie-breaker to total-order | Logical only, no wall clock needed | Detects concurrency, but alone doesn't total-order a linear history | **Use as detection, not as final ordering** |
| **Hybrid Logical Clocks (HLC) + LWW-map** | Every op gets `HLC(pt: u64 ms, counter: u16, node_id: PeerId)` where `node_id = PeerId` bytes; HLC is causally consistent and bounded from wall time; LWW-Map picks `max(HLC)` per key; product of CRDTs is a CRDT (CAI) [Source: `did:crdt arXiv 2606.16223 — HLC triple (physical_ms, counter, node_id derived from public key), deterministic tiebreaker, product of CRDTs is CRDT`](https://arxiv.org/html/2606.16223) ; `Whispering-app — HLC is wall-clock LWW with deterministic tiebreaker; preserves causality`](https://github.com/Tr0py/whispering-app/blob/main/docs/articles/crdt-conflict-resolution-strategies.md) | Totally ordered, deterministic, causal | Tolerates bounded skew (ms) | Same LWW drop issue if used naïvely per-register | **Use for per-op ordering key, not as sole fork-choice** |
| **Deterministic longest-valid-history (recommended)** | History = ordered list of `SignedBatch { seq, batches: Vec<Transfer> }` where each batch locally validates (`sum==SUPPLY` when genesis included, nonce monotonic, no negative balances). Fork-choice picks the history with (1) highest `max_acked_seq` that is fully valid, tiebreak (2) lowest `blake3(canonical_history_bytes)`, tiebreak (3) smallest lexicographic `PeerId` of tip author. Invalid batches are never acked and cause gossipsub `Validation::Reject` + peer penalty. | Fully deterministic given same input set; converges without clocks | No wall clock needed for ordering (seq counters only) | **Yes — validity is the primary filter** | **Recommended for AI Bank MVP** |
| **Reputation-weighted fork-choice** | Weight history by sum of signer reputations or tip signer's reputation (see #11). Highest-weight history wins. | Deterministic if reputation is deterministic | No clock | Tempting but Sybil-amplifiable at FAL-2 (N=50 Sybil harness shows reputation distortion); creates circular dependency ledger→reputation→fork-choice→ledger | **Defer to Phase-2; display-only at FAL-2** per ADR 0004 — `tenure-weighted decay + diversity + whitewash cost` not yet tunable per #11 |

### 3.2 Recommended Fork-Choice for MVP (Deterministic Longest-Valid-History)

```text
valid(history) :=
  history[0] == genesis  ∧
  ∀ batch in tail(history): verify_batch(batch) == Ok
  where verify_batch checks (in this order, fail-fast):
    1. Ed25519 sig over domain b"/ai-bank/1/" verifies against batch.from PeerId
    2. batch.nonce == expected_nonce[from]  (replay protection)
    3. 0 < amount ≤ MAX_AMOUNT  and amount ≥ DUST_THRESHOLD
    4. apply_on_copy(state, batch): no negative balances
    5. sum(all_balances) == SUPPLY  (supply invariant — O(accounts) but batched; see §4)

fork_choice(histories) :=
  let valid = histories.filter(valid)
  in if valid.empty() → keep current (or genesis if none)
     else valid.max_by_key(|h| (h.tip_seq, -blake3(canon(h)), -h.tip_author_bytes))
     // lexicographic: highest tip_seq first; tie → lowest hash; tie → lowest PeerId
checkpoint: social fork via signed `Checkpoint{ seq, history_hash, signers: Vec<PeerId> }`
  with supermajority of known peers (or maintainer key for FAL-2) pinned as new genesis anchor
```

**Why this ordering of tiebreakers:**

- `tip_seq` is the logical clock (monotonic per-author counter, aka Hybrid Logical Clock's `counter` stripped to sequence number). No wall-clock skew sensitivity, avoids `agravier/crdt-study` "LWW requires extrinsic sync" pitfall [Source: `crdt-study — tolerates skew unless vector clock or NTP`](https://github.com/agravier/crdt-study).
- `blake3` hash tiebreaker is unbiased and key-independent (unlike `PeerId` alone, which would always favor low-byte peers).
- `PeerId` final tiebreaker guarantees total order without randomness — important for `cargo test` determinism per ADR 0006.

**What about vector clocks?** Use a lightweight vector clock *inside* each `SignedBatch` as causal metadata, not as ordering:

```rust
struct SignedBatch {
    from: PeerId,
    seq: u64,                    // per-sender monotonic — the HLC counter
    vclock: BTreeMap<PeerId, u64>, // sparse vector clock (only peers seen)
    depends_on: Vec<(PeerId, u64)>, // explicit causal predecessors (optional)
    transfers: Vec<Transfer>,   // amount, to, memo
    sig: Vec<u8>,               // Ed25519 over b"/ai-bank/1/batch:" || cbor(batch_without_sig)
}
```

Nodes track `max_seen: BTreeMap<PeerId, u64>` and flag `V_a || V_b` as concurrent for debugging/metrics, but do **not** let vector-clock dominance override validity. Valid history length wins.

**Reputation-weighted variant (deferred):** When #11 defines tenure-weighted decay, replace `tip_seq` with `score = tip_seq + α * sum_reputation(history)` where `α` is small (e.g., 0.01) and reputation is computed *from* the candidate history itself (so weight is endogenous). Do **not** ship this at FAL-2 — keep fork-choice reputation-free to avoid the circular `ledger → reputation → fork-choice` loop that the Sybil N=50 harness is designed to catch (ADR 0006). Document as `FAL-3 candidate` behind a feature flag.

### 3.3 Equivocation and Double-Spend Handling

Without a chain, double-spend = two batches from same `from` with same `nonce` (or `nonce` reused) or two batches that both spend the same funds in different histories.

- **Same nonce, different payload → equivocation proof.** First valid batch at that nonce is kept; second is `Validation::Reject` and the `from` peer's P₄ score penalised via `PeerScoreParams`. The equivocating signed batch pair is gossipped as evidence (small `EquivocationProof{ from, nonce, batch_a, batch_b }` on a separate gossipsub topic `/ai-bank/evidence/1.0.0`) so all peers slash locally.
- **Overspend across histories → only one history survives.** Both histories may be locally valid in isolation (they each passed `apply_on_copy` against their own predecessor state), but only one wins `fork_choice`. The losing history's tail batches are discarded; their senders see `InsufficientFunds` on replay. This matches Bitcoin's "longest valid chain" without PoW — here "valid" does the work, not "longest" alone.

### 3.4 Checkpoint / Social-Fork Recovery

At FAL-2, ADR 0004/0006 accept that worst-case corruption is recovered by social fork, not by automatic BFT finality.

- **Checkpoint record:** `Checkpoint{ seq, history_hash: Blake3, signers: Vec<(PeerId, Sig)> }` signed over `b"/ai-bank/1/checkpoint:" || seq || history_hash`. Nodes pin the checkpoint hash as a new trusted anchor; fork-choice is constrained to histories that extend the pinned checkpoint (short-circuit eval before `max_by_key`).
- **Who signs?** MVP: maintainer key (ADR 0005 governance) + any quorum of online peers where quorum = `ceil(known_peers / 2)` for liveness on small networks. Do not require Kademlia quorum (`Quorum::N` over DHT) — ledger liveness must not depend on DHT liveness.
- **How distributed:** Publish checkpoint on `gossipsub` topic `/ai-bank/checkpoint/1.0.0` + embed in release tag `checkpoint/YYYY-MM-DD` + Rekor attestation per ADR 0005/0006. Nodes verify sig and pin.
- **Recovery after pause tag:** `git tag -s pause/YYYY-MM-DD` (ADR 0006 emergency pause) is itself a checkpoint with zero history beyond the tag.

---

## 4. Supply Invariant, Nonce, Dust, and Signing Domain

### 4.1 Genesis Table and Supply Invariant

| Item | Value / shape | Source / rationale |
|---|---|---|
| **Total supply `SUPPLY`** | **TBD by #10 + #12 (grilling)** — recommendation: `u64` smallest unit (e.g., 1_000_000_000 with 6 decimal places → 1e9 * 1e6 = 1e15 fits in u64). Single constant `const SUPPLY: u64 = ...` compiled into `genesis.json` and verified by hash, not negotiated at runtime. | FAL-2 fixed-supply per ADR 0004; u64 fits all real ledgers; larger needs u128 but complicates dust math |
| **Genesis artifact** | `genesis.json` (or `genesis.cbor`) = `{ supply: u64, balances: BTreeMap<PeerId, u64>, checkpoint_seq: 0, history_hash: Blake3(genesis), sig: Ed25519(maintainer_key, b"/ai-bank/1/genesis:" || blake3(canon(genesis_without_sig))) }`. Hash-pinned (`blake3(canon(genesis))`) at compile time and in checkpoint, not fetched over DHT at start without verification. | ADR 0004 genesis-mint-only + ADR 0006 supply-invariant test requirement |
| **Allocation** | Options remain open per #12: equal split among bootstrap peers, file-based distribution list, or invite code — all produce the same `BTreeMap<PeerId, u64>` artifact. Choice is social/governance, not transport — this doc notes only that whatever the map, it must be committed as the genesis artifact. | Ticket #12 blocks allocation decision; this doc does not pre-decide |
| **Bootstrap for new node** | Ship `genesis.json` with binary (or fetch over HTTPS from release + verify sig + hash-pin) → verify `sum(balances)==SUPPLY` locally → insert as seq-0 history → then enter DHT/gossipsub swarm. Never trust a DHT `get_record` genesis without sig check. | Same walkaway test as `identity.key` per ADR 0001 |
| **Sum check per batch** | `debug_assert!(state.values().map(|b| b as u128).sum::<u128>() == SUPPLY as u128)` after every `apply_batch` in tests; in production, check *after* applying batch on a copy, reject batch if `sum != SUPPLY`. Use `u128` accumulator to avoid wrapping before compare. Batching: verify once per batch (e.g., 64 transfers), not per transfer, to keep O(n) sum cheap. Alternatives: maintain running `u64 total` that must equal `SUPPLY` after each transfer — constant time but requires correct init. | FAL-2 safeguard per ADR 0006 harness |
| **Per-batch cost** | Naïve sum is O(accounts). For 10k accounts, ~80µs per batch on redb (B-tree scan). Mitigate by keeping `total: u64` in state and asserting `total == SUPPLY` after apply, plus periodic full-scan audit (every N batches or on checkpoint) that recomputes sum from storage. | Trade-off: strong invariant vs scan cost |

### 4.2 Nonce / Replay Protection

- **Per-sender monotonic `u64` seq (aka nonce):** `batch.seq == expected[from]` where `expected[from]` = `max_seq[from] + 1` (1-indexed; 0 is genesis). Store `highest_seq: Table<PeerId, u64>` in `redb`/`sled`. Reject any batch with `seq <= highest[from]` as replay; `seq == highest+1` is next expected; `seq > highest+1` is gap — buffer or request missing seq via request-response sync (B).
- **Why per-sender, not global:** Global seq needs a sequencer (blockchain). Per-sender is CRDT-friendly (each PeerId is its own log) and fork-choice merges logs deterministically; verification is O(1) per batch.
- **Persistence of nonce map:** Same table as balances, same transaction — atomic `write_batch { balances, nonces }` so a crash cannot advance nonce without persisting balance change.
- **HLC vs plain seq:** HLC `(pt, counter, node_id)` is strictly more expressive but heavier; plain `seq` suffices for ordering; use HLC only if wall-clock causality debugging is needed. Recommend plain `seq` for MVP, HLC as optional extension field.

### 4.3 Dust Threshold

- **Purpose:** Prevent DoS via 1-unit transfers that bloat storage and peer scoring, and prevent `sum==SUPPLY` bypass via rounding if fractional units existed (they don't — u64 integer).
- **Threshold:** Constant `DUST_THRESHOLD: u64` (e.g., 100 smallest units = $0.0001 at 6 decimals) compiled or in `genesis.json` config. Reject `amount < DUST_THRESHOLD` at `validate_messages` gate before `Validation::Accept` (also apply at request-response inbound). ADR 0006 dust flood test (10× rate) exercises this.
- **Interaction with supply:** Dust reject is *before* apply, so it cannot violate supply away from invariant; document that dust is a local policy, not a consensus split — two nodes with different thresholds still agree on supply but may disagree on which small transfers are gossipped; set threshold in genesis to keep policy consensus (treat mismatch as soft fork, resolved by checkpoint).

### 4.4 Signing Domain `b"/ai-bank/1/"`

Reuses ADR 0001's Ed25519 domain separation verbatim to avoid cross-protocol replay:

```
sig = Ed25519.sign(sk,  b"/ai-bank/1/batch:" || cbor(canonical_batch_without_sig))
sig = Ed25519.sign(sk,  b"/ai-bank/1/transfer:" || cbor(canonical_transfer_without_sig))  // if per-transfer sig
sig = Ed25519.sign(sk,  b"/ai-bank/1/checkpoint:" || seq_be || history_hash)
sig = Ed25519.sign(sk,  b"/ai-bank/1/genesis:" || blake3(canon(genesis_without_sig)))
```

- **Key binding:** Verify with sender's `PeerId` public key via `libp2p_identity::PublicKey::try_from_protobuf_encoding` → `PeerId::from_public_key` → `verify(domain || payload, sig)` [Source: `ADR 0001` identity domain + `libp2p-identity` `Keypair` / `ed25519_dalek` interop note].
- **Canonical encoding:** Use `cbor4ii` or `serde_cbor` with deterministic map key ordering (BTreeMap) or `ciborium` canonical mode; cross-check with `schemars` JSON schema for axum/mcp adapters per ADR 0003/#10. The same CBOR is used for `gossipsub` payload and `request-response` codec so validation can be shared.
- **Gossipsub message authenticity:** Configure `MessageAuthenticity::Signed(keypair)` so libp2p itself signs the gossip envelope; *also* sign the application payload with `b"/ai-bank/1/"` — two layers: transport-level `PeerId` binding (Noise/TLS handshake + gossipsub message sig) plus application-level domain-separated batch sig that survives storage and sync RPC without the gossip envelope.

---

## 5. Transport Wiring: How Gossipsub + Request-Response Compose on the Swarm

```rust
use libp2p::{gossipsub, identify, kad, mdns, request_response::cbor, swarm::NetworkBehaviour};

#[derive(NetworkBehaviour)]
struct BankBehaviour {
    gossipsub: gossipsub::Behaviour,
    request_response: cbor::Behaviour<SyncRequest, SyncResponse>,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
    // relay, dcutr, autonat per ADR 0002
}
```

- **Gossipsub setup (strict, signed):**
  ```rust
  let gossipsub = {
      let mut cfg = gossipsub::ConfigBuilder::default()
          .heartbeat_interval(Duration::from_secs(1))
          .mesh_n(6).mesh_n_low(4).mesh_n_high(12)
          .gossip_factor(0.25)
          .history_length(5).history_gossip(3)
          .validation_mode(gossipsub::ValidationMode::Strict)
          .validate_messages(true)
          .max_transmit_size(64 * 1024)
          .build()?;
      let authenticity = gossipsub::MessageAuthenticity::Signed(keypair.clone());
      gossipsub::Behaviour::new(authenticity, cfg)?
  };
  let topic = gossipsub::IdentTopic::new("/ai-bank/transfer/1.0.0");
  gossipsub.subscribe(&topic)?;
  // also /ai-bank/evidence/1.0.0 and /ai-bank/checkpoint/1.0.0
  ```
  Defaults from [`ConfigBuilder` docs — mesh_n=6, heartbeat 1s, factor 0.25, Strict validation](https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Config.html) ; [`Behaviour::new` requires MessageAuthenticity + Config](https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Behaviour.html) ; [`ValidationMode::Strict`](https://docs.rs/libp2p/latest/libp2p/gossipsub/enum.ValidationMode.html) rejects unsigned/invalid sender.

- **Request-response setup (CBOR):**
  ```rust
  let rr_cfg = cbor::Config::default()
      .with_request_timeout(Duration::from_secs(10));
  let rr = cbor::Behaviour::<SyncRequest, SyncResponse>::new(
      [ (StreamProtocol::new("/ai-bank/sync/1.0.0"), ProtocolSupport::Full) ],
      rr_cfg,
  );
  ```

- **Event loop sketch (single handler, transport-blind service layer):**
  ```rust
  match swarm.next().await {
      SwarmEvent::Behaviour(BankBehaviourEvent::Gossipsub(gossipsub::Event::Message{ message, .. })) => {
          let batch: SignedBatch = cbor4ii::serde::from_slice(&message.data)?;
          match validate_batch(&state, &batch) {
              Ok(()) => {
                  swarm.behaviour_mut().gossipsub
                      .report_message_validation_result(&msg_id, &propagation_source, gossipsub::MessageAcceptance::Accept);
                  apply_batch(&mut state, batch); // redb transaction
              }
              Err(e) if e.is_equivocation() => {
                  swarm.behaviour_mut().gossipsub
                      .report_message_validation_result(&msg_id, &propagation_source, gossipsub::MessageAcceptance::Reject);
                  publish_evidence(proof);
              }
              Err(_) => {
                  swarm.behaviour_mut().gossipsub
                      .report_message_validation_result(&msg_id, &propagation_source, gossipsub::MessageAcceptance::Reject);
              }
          }
      }
      SwarmEvent::Behaviour(BankBehaviourEvent::RequestResponse(rr::Event::Message{ peer, message })) => match message {
          rr::Message::Request{ request, channel, .. } => {
              let resp = handle_sync(request); // read redb
              let _ = swarm.behaviour_mut().request_response.send_response(channel, resp);
          }
          rr::Message::Response{ response, request_id } => {
              for batch in response.batches { let _ = validate_and_apply(batch); }
          }
      },
      // identify → kad hook per #8
      SwarmEvent::Behaviour(BankBehaviourEvent::Identify(identify::Event::Received{ peer_id, info })) => {
          if info.protocols.contains(&kad::PROTOCOL_NAME) {
              for addr in info.listen_addrs { swarm.behaviour_mut().kademlia.add_address(&peer_id, addr); }
          }
      }
      _ => {}
  }
  ```

---

## 6. What Depends on #10 (Data Model) — Deferred Record Shape

This research is intentionally transport-layer only. The following are **blocked on #10** and must be decided there, then plugged back into this design without changing the transport choice:

| Deferred item | Why it blocks §4 | Constraint this doc imposes |
|---|---|---|
| **Account ID** — `PeerId` directly vs derived `AccountId` vs delegation `node_key → account` | Determines `balance_table` key type and delegation verification in `validate_batch` | Must be derivable from `PeerId` or a signature chain anchored to `PeerId`; no separate CA |
| **Transfer schema field order + encoding** — CBOR vs JSON, `schemars` derive, per-transfer vs per-batch sig | Determines canonical bytes for `b"/ai-bank/1/"` sig and redb value layout | Use CBOR with deterministic map ordering for on-wire + storage; derive `schemars::JsonSchema` for axum/rmcp adapters per ADR 0003; batch sig over canonical batch bytes |
| **Amount type** — `u64` vs `u128`, denomination, `MAX_AMOUNT`, dust granularity | Determines `total: u128` accumulator check and `redb::TableDefinition` value type | Recommend `u64` for accounts (fits `SUPPLY`), `u128` only for accumulator; dust and max as `u64` constants |
| **Timestamp field** — wall-clock vs absent vs HLC | Determines whether fork-choice needs clock validation | Recommend no wall-clock in consensus path; optional `HLC` extension field only |
| **Balance + nonce storage layout** — separate tables vs single composite value | Determines `redb` transaction atomicity | Must be atomic per batch (single `write_txn` covering both tables) |

**Handoff contract with #10:** When #10 lands, update this doc's §4 examples to import its `Transfer`, `SignedBatch`, `AccountId` types verbatim; no transport code needs to change — only validation and storage value types.

---

## 7. Storage: `redb` vs `sled` for the Local Durable Log

### 7.1 Comparison Table

| Dimension | `redb` (recommended) | `sled` |
|---|---|---|
| **Model** | Pure Rust, copy-on-write B+-trees, LMDB-inspired, collection of tables in one file [Source: `redb 3.1.0 docs — "collection of copy-on-write B-trees", design doc`](https://docs.rs/crate/redb/3.1.0) | Lock-free Bw-tree + log-structured merge over fragments, `Tree`/`Db` with keyspace `BTreeMap<[u8],[u8]>` API [Source: `sled 0.34.7 docs — "flash-sympathetic lock-free B+ tree", architectural outlook`](https://docs.rs/crate/sled/0.34.7) |
| **Correctness** | Fully ACID, MVCC concurrent readers + one writer without blocking, crash-safe by default, savepoints/rollback, `write_txn.commit()` fsyncs [Source: `redb docs — Features: ACID, MVCC, crash-safe`](https://docs.rs/crate/redb/3.1.0) | Atomic single-key ops + `compare_and_swap`, serializable multi-key `Tree::transaction`, `apply_batch`, `flush`/`flush_async`; but crate advertises beta with known issues [Source: `sled docs — features, transactions, known issues "if reliability is your primary constraint, use SQLite. sled is beta"`](https://docs.rs/crate/sled/0.34.7) |
| **File format stability** | **Stable since 1.0 (2023-06-16); reasonable effort to provide upgrade path** [Source: `redb — Status: stable and maintained, file format stable since 1.0`](https://docs.rs/crate/redb/3.1.0) | **Unstable before 1.0; manual `export` migration required between alpha releases**; `0.34.7` last stable (2021-09-12) with 1.0 still in `alpha.124` [Source: `sled docs — "on-disk format is going to change in ways that require manual migrations before 1.0.0"` + versions table](https://docs.rs/crate/sled/0.34.7) |
| **Maintenance** | Actively maintained (C. Berner), releases up to `4.2.0` (2026-08), 1.17M downloads/mo, used in 853 crates [Source: `crates.io redb 4.2.0`](https://crates.io/crates/redb) ; benchmarks vs lmdb/rocksdb/sled show competitive throughput [Source: `redb benchmarks table — redb vs lmdb vs sled`](https://docs.rs/crate/redb/3.1.0#benchmarks) | Last stable `0.34.7` (2021); 1.0 alphas since 2023 but beta warnings remain; community notes intermittent maintenance ["abandoned for some years, author working on new engine"](https://www.libhunt.com/compare-sled-vs-redb) — still usable but higher supply-chain risk |
| **API for ledger** | `Database::create("ai-bank.redb")` → `write_txn.open_table(TableDefinition)` → `table.insert(k,v)` → `write_txn.commit()`; typed tables `TableDefinition<PeerIdBytes, u64>`; range scans via `table.range(..)` | `sled::open("ai-bank.sled")` → `db.insert(k,v)` / `db.apply_batch(batch)` / `db.transaction(|tx_db| ...)`; prefix scans `db.range(..)` / `db.scan_prefix(..)` |
| **Concurrency** | Single writer, many readers via MVCC — matches ledger's single-writer-per-node model (append batches sequentially) | Lock-free, cpu-scalable — best when many concurrent writers contending |
| **Disk usage** | Compacted size ~1.69 GiB on benchmark dataset (vs sled 2.13 GiB uncompacted) [Source: `redb benchmarks — uncompacted/compacted size`](https://docs.rs/crate/redb/3.1.0#benchmarks) | Uses extra space sometimes ["sled uses too much disk space sometimes"](https://docs.rs/crate/sled/0.34.7#performance) ; no compaction in stable branch |
| **MSRV / deps** | `libc` only; pure Rust, portable, no native deps; light `Database` handle | `crossbeam-epoch`, `parking_lot`, `fs2`, `crc32fast` — also pure Rust, but more deps |

### 7.2 Recommended Ledger Schema on `redb`

```rust
use redb::{Database, TableDefinition, ReadableTable};

const BALANCES: TableDefinition<&[u8], u64> = TableDefinition::new("balances");      // key: PeerId bytes
const NONCES:   TableDefinition<&[u8], u64> = TableDefinition::new("nonces");        // key: PeerId bytes → highest seq
const BATCHES:  TableDefinition<u64, &[u8]> = TableDefinition::new("batches");       // key: global seq (if linearized) or (PeerId,seq) composite; value: CBOR SignedBatch
const META:     TableDefinition<&str, &[u8]> = TableDefinition::new("meta");         // "supply", "genesis_hash", "checkpoint_seq"

fn apply_batch(db: &Database, batch: &SignedBatch) -> Result<(), ApplyError> {
    let txn = db.begin_write()?;
    {
        let mut balances = txn.open_table(BALANCES)?;
        let mut nonces = txn.open_table(NONCES)?;
        // ... validate nonce, balances, sum==SUPPLY on in-memory copy ...
        // ... then write mutated balances + nonces ...
        let mut batches = txn.open_table(BATCHES)?;
        batches.insert(batch.seq, cbor4ii::serde::to_vec(batch)?.as_slice())?;
    }
    txn.commit()?; // ACID — nonce+balances+batches atomically persisted
    Ok(())
}
```

- One `write_txn` per batch (or per gossipsub message if messages carry one batch) — the ledger writer is single-threaded per node, so the `redb` single-writer limit is not a bottleneck.
- Readers (`begin_read`) are concurrent and non-blocking for reputation computation (#11) and agent API reads (ADR 0003) via `axum` handler that shares the `Database` handle.
- Durability: `commit()` is durable; no explicit `flush` needed. For sled the equivalent is `db.flush()` / `flush_async().await` [Source: `sled docs — "block until all operations are stable on disk (flush_async also available)"`](https://docs.rs/crate/sled/0.34.7).

### 7.3 When `sled` Would Be Preferred

Choose `sled` over `redb` only if benchmarks on the actual transfer shape show:

- Many concurrent writers (e.g., multiple Tokio tasks applying batches in parallel — but ledger ordering forbids parallel apply anyway), **and**
- Tolerance for beta format-instability + `export` migration before 1.0 is explicitly accepted by maintainers.

Otherwise, default to `redb`.

---

## 8. Crate Map & Verification

### 8.1 Crate Map (current at 2026-09, per `crates.io` + `docs.rs`)

| Crate | Latest stable | MSRV / note | Needed for ledger | Source |
|---|---|---|---|---|
| `libp2p-gossipsub` | **0.49.5** (2026-07-21 build glitch), last good **0.49.2** (2025-08-05); **0.49.0** (2025-06-27) | MSRV 1.83 via umbrella | Mesh replication | [`crates.io libp2p-gossipsub 0.49.0`](https://crates.io/crates/libp2p-gossipsub/0.49.0) + [`docs.rs libp2p-gossipsub 0.49.0`](https://docs.rs/crate/libp2p-gossipsub/0.49.0) |
| `libp2p-request-response` | **0.29.x** in umbrella 0.56 | MSRV 1.83 | Sync catch-up RPC | [`lib.rs libp2p-request-response — cbor/json behaviours`](https://lib.rs/crates/libp2p-request-response) + [`docs.rs libp2p::request_response::cbor`](https://docs.rs/libp2p/latest/libp2p/request_response/cbor/index.html) |
| `libp2p` umbrella | **0.56.0** (2025-06-28) — bundles gossipsub 0.49, request-response 0.29, kad 0.48, swarm 0.47 etc. | 1.83.0 | Single version pin | [`libp2p.io releases 2025-06-28`](https://libp2p.io/releases/2025-06-28-rust-libp2p/) + `crates.io libp2p` |
| `redb` | **4.2.0** (2026-08-17) stable format since 1.0; **3.1.0** (2025-09-25) also stable | Pure Rust | Local ledger log (recommended) | [`crates.io redb`](https://crates.io/crates/redb) + [`docs.rs redb 3.1.0`](https://docs.rs/crate/redb/3.1.0) |
| `sled` | **0.34.7** (2021-09-12) stable; `1.0.0-alpha.124` pre-release | 1.39+ | Alternative local log (deferred) | [`crates.io sled 0.34.7`](https://crates.io/crates/sled/0.34.7) + [`docs.rs sled`](https://docs.rs/crate/sled/0.34.7) |
| `libp2p-identity` / `libp2p-core` | Re-exported via umbrella | Ed25519 `PeerId`, `Keypair` | Sig verify | `ADR 0001` |
| `cbor4ii` / `serde_cbor` / `ciborium` | `cbor4ii 0.3.x` | CBOR codec for both gossipsub payload and request-response | Canonical transfer encoding | `libp2p-request-response cbor` docs |

Pin MVP to **`libp2p 0.56 + libp2p-gossipsub 0.49 + libp2p-request-response 0.29 + redb 3.1/4.x` (MSRV 1.83)**. `redb 4.x` is an in-place upgrade from `3.x` (stable format, major = API break only); verify with `cargo tree | grep -E "redb|libp2p-gossipsub|libp2p-request-response"`.

### 8.2 Feature Flags on `libp2p`

```toml
[dependencies]
libp2p = { version = "0.56", features = [
  "tokio", "tcp", "quic", "dns", "noise", "yamux",
  "identify", "kad", "gossipsub", "request-response",
  "relay", "dcutr", "autonat", "mdns"
]}
redb = "4.1"
cbor4ii = { version = "0.3", features = ["serde"] }
blake3 = "1"
ed25519-dalek = "2"
```

Avoid `features = ["full"]` in production (pulls wasm transports). Exact flag list lives at [`docs.rs/crate/libp2p/0.56/features`](https://docs.rs/crate/libp2p/0.56/features).

### 8.3 Verification Commands (copy-paste)

```bash
# 1) crates.io metadata (no clone needed)
cargo search libp2p-gossipsub --limit 3
cargo search libp2p-request-response --limit 3
cargo search redb --limit 3
cargo info libp2p-gossipsub@0.49.2   # MSRV + deps + repository
cargo info redb@4.1.0

# 2) local pin check after Cargo.toml edit
cargo tree | grep -E "libp2p-gossipsub|libp2p-request-response|redb"
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="redb") | {version, rust_version}'
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="libp2p-gossipsub") | {version, rust_version}'

# 3) docs.rs as source of truth per-field
# https://docs.rs/libp2p-gossipsub/0.49.0/libp2p_gossipsub/struct.Behaviour.html
# https://docs.rs/libp2p-gossipsub/0.49.0/libp2p_gossipsub/struct.Config.html
# https://docs.rs/libp2p-gossipsub/0.49.0/libp2p_gossipsub/config/struct.ConfigBuilder.html
# https://docs.rs/libp2p/latest/libp2p/gossipsub/enum.ValidationMode.html
# https://libp2p.github.io/rust-libp2p/libp2p/request_response/struct.Behaviour.html
# https://docs.rs/crate/redb/3.1.0  # stable status + benchmarks
# https://docs.rs/crate/sled/0.34.7 # beta warning + flush API

# 4) specs as authority for gossipsub semantics
# https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md
# https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md  # peer scoring
# https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.2.md  # IDONTWANT
```

---

## 9. Recommendations for AI Bank Ledger on This Stack

1. **Primary replication = `gossipsub` with strict signed validation:** `IdentTopic("/ai-bank/transfer/1.0.0")`, `MessageAuthenticity::Signed`, `ValidationMode::Strict`, `validate_messages(true)`, `mesh_n=6`. Publish CBOR `SignedBatch`; subscriber handles `Event::Message` → `validate_batch` → `report_message_validation_result(Accept/Reject)` → `redb::write_txn`. Map invalid-batch and equivocation to `Reject` + P₄/P₅ penalty so gossipsub mesh ejects the sender.

2. **Catch-up = `request-response CBOR` for anti-entropy:** Protocol `/ai-bank/sync/1.0.0` with `SyncRequest{since_seq, limit}` → `SyncResponse{batches}`. Trigger on reconnect, on gap (`seq > expected+1`), and on periodic timer (e.g., every 30s or every 10 gossipsub heartbeats) to heal partitions beyond `mcache` window. Use `send_request_with_addresses` if kad hasn't yet learned the peer's addrs.

3. **Keep Kademlia out of the ledger critical path:** DHT remains for peer routing per #8 / ADR 0002 (`get_closest_peers`, `put_record` for alias hints only). Do not store transfers as DHT records; do not use `start_providing` for transfers.

4. **Fork-choice = deterministic longest-valid-history with validity-first filter:** Validity checks in order: domain-separated sig → nonce monotonic → dust → no-negative-balances → `sum==SUPPLY`. Winner is `(valid, highest tip_seq, lowest blake3(canonical_history), lowest PeerId)`. Vector clocks stored as metadata for concurrency telemetry, not as ordering. Reputation-weighted choice deferred to FAL-3 behind a flag (see #11).

5. **Supply invariant enforced twice:** (a) fast path `total: u64` field in state, asserted `== SUPPLY` after every batch apply; (b) periodic full-scan audit every N batches and on every checkpoint load, iterating `BALANCES` table via `redb` range scan and summing in `u128`. Both paths covered by `cargo test` in ADR 0006 harness.

6. **Signing domain `b"/ai-bank/1/"` for all ledger artifacts:** Reuse ADR 0001's domain verbatim: `b"/ai-bank/1/batch:"`, `b"/ai-bank/1/checkpoint:"`, `b"/ai-bank/1/genesis:"` concatenated with canonical CBOR / hash before `ed25519_dalek::SigningKey::sign`. CBOR must be canonical (BTreeMap key order via `cbor4ii` deterministic mode).

7. **Local persistence = `redb`:** `Database::create("ai-bank.redb")` with tables `balances`, `nonces`, `batches`, `meta`; one `write_txn` per batch covering balances+nonces+batches atomically; `read_txn` concurrent for API/reputation reads. Document that `sled` remains an alternative only if lock-free concurrency is later proven dominant and format-instability is accepted.

8. **Checkpoint/social-fork plan:** Publish `Checkpoint{seq, history_hash, sigs}` on `/ai-bank/checkpoint/1.0.0` + release tag + Rekor; nodes pin `history_hash` and constrain future `fork_choice` to histories that extend it. Emergency `pause/YYYY-MM-DD` tag is itself a checkpoint with zero tail.

9. **Dependency on #10:** Record shape (`Transfer`, `SignedBatch`, `AccountId` types, `DUST_THRESHOLD`, `MAX_AMOUNT`, denomination, CBOR vs JSON, `schemars` schemas, delegation chain) deferred to #10; this doc's §4/§5 examples are placeholder names that must be replaced by importing #10's canonical types. Track the handoff as a blocked-by edge: close #10 before finalising ledger crate `types` module.

---

## 10. Open Questions for Maintainer (Grilling / Decision Needed)

- **Supply denomination** — will `u64` at 6 decimals (`SUPPLY` e.g. 1_000_000__000_000) be pinned in `genesis.json` now (option A), or should `SUPPLY` stay configurable per network (option B, supply as genesis param)? Option A simplifies `sum==SUPPLY` as a compile-time constant; option B allows testnets.
- **Dust threshold** — must it be a consensus constant (in genesis, part of supply-invariant validation, hard-forks if changed) or a local policy (each node may reject small transfers but still accept histories containing them)? Recommendation: genesis constant for MVP (simplest), local policy later.
- **Reputation in fork-choice** — confirm deferral to FAL-3 (this doc's §3.4 α-weighted variant stays behind a feature flag, off by default, so `cargo test` determinism is not coupled to reputation tuning from #11). If maintainers want reputation-weighted at MVP, define α and reputation formula in #11 first.
- **Checkpoint quorum** — maintainer-key-only pin (simplest for small network) vs `ceil(n/2)` peer supermajority vs both (maintainer signature mandatory + peer quorum supplementary). For FAL-2 with contained blast radius, maintainer-key-only is proportionate; peer quorum can be added at FAL-3.
- **Batch size cap** — max transfers per `SignedBatch` and max `max_transmit_size` for gossipsub (64 KiB default → ~100 transfers at ~500B each). Cap must be smaller than `max_transmit_size` minus protobuf overhead.

---

## Sources — Primary Only (Every Claim Traces Above)

- **`specs/pubsub/gossipsub/gossipsub-v1.0.md`** — mesh, fanout, GRAFT/PRUNE, IHAVE/IWANT, heartbeat, mcache [https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md)
- **`specs/pubsub/gossipsub/gossipsub-v1.1.md`** — peer scoring P₁–P₇, thresholds, opportunistic grafting, decay to zero [https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md)
- **`specs/pubsub/gossipsub/gossipsub-v1.2.md`** — IDONTWANT aggregation [https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.2.md](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.2.md)
- **`specs/pubsub/gossipsub/README.md`** — implementation status table (rust-libp2p v1.0+v1.1 done, v1.2 in progress) [https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/README.md](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/README.md)
- **`docs.rs libp2p-gossipsub 0.49.0`** — crate overview, MessageAuthenticity, Behaviour::new, Config/ConfigBuilder defaults (mesh_n=6, heartbeat 1s) [https://docs.rs/crate/libp2p-gossipsub/0.49.0](https://docs.rs/crate/libp2p-gossipsub/0.49.0)
- **`docs.rs libp2p::gossipsub Config`** — `mesh_n/mesh_n_low/mesh_n_high`, `history_length`, `validate_messages` ("prevents automatic forwarding — must call report_message_validation_result"), `ValidationMode` [https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Config.html](https://docs.rs/libp2p/latest/libp2p/gossipsub/struct.Config.html)
- **`libp2p.github.io Behaviour::report_message_validation_result` + `MessageAcceptance::Accept/Reject/Ignore`** — validation gate semantics [https://libp2p.github.io/rust-libp2p/libp2p/gossipsub/struct.Behaviour.html](https://libp2p.github.io/rust-libp2p/libp2p/gossipsub/struct.Behaviour.html)
- **`docs.rs / lib.rs libp2p-request-response + cbor`** — `cbor::Behaviour` alias ("using cbor4ii::serde"), `Codec` generic, protocol families as sum types [https://docs.rs/crate/libp2p-request-response/latest](https://docs.rs/crate/libp2p-request-response/latest) ; [https://lib.rs/crates/libp2p-request-response](https://lib.rs/crates/libp2p-request-response)
- **`libp2p.github.io request_response Behaviour::send_request/send_request_with_addresses/Config`** — new substream per request, dial-or-discovery note, address-aware variant [https://libp2p.github.io/rust-libp2p/libp2p/request_response/struct.Behaviour.html](https://libp2p.github.io/rust-libp2p/libp2p/request_response/struct.Behaviour.html)
- **`libp2p.io guides/dht` + `libp2p.io docs/kademlia-dht` + `specs/kad-dht/README.md`** — Kademlia K=20, α=3, provider vs value records, expiry/republish [https://libp2p.io/guides/dht/](https://libp2p.io/guides/dht/) ; [https://libp2p.io/docs/kademlia-dht/](https://libp2p.io/docs/kademlia-dht/)
- **`crdt-study (agravier) — LWW study`** — LWW requires unique totally-ordered timestamps consistent with causal order; needs monotonic clock + NTP or vector clock for skew tolerance [https://github.com/agravier/crdt-study](https://github.com/agravier/crdt-study)
- **`LowLevelDesignMastery — Vector clocks`** — V1<V2 iff ∀i ≤ ∧ ∃i < ; V1‖V2 concurrent definition [https://www.lowleveldesignmastery.com/hld-concepts/consistency/05-conflict-resolution/](https://www.lowleveldesignmastery.com/hld-concepts/consistency/05-conflict-resolution/)
- **`GeeksforGeeks — Vector clocks in distributed systems`** — vector clocks establish sequence without world clock [https://www.geeksforgeeks.org/computer-networks/vector-clocks-in-distributed-systems/](https://www.geeksforgeeks.org/computer-networks/vector-clocks-in-distributed-systems/)
- **`Tr0py/whispering-app — CRDT conflict resolution`** — HLC is wall-clock LWW with deterministic tiebreaker; preserves causality; node_id as tiebreaker [https://github.com/Tr0py/whispering-app/blob/main/docs/articles/crdt-conflict-resolution-strategies.md](https://github.com/Tr0py/whispering-app/blob/main/docs/articles/crdt-conflict-resolution-strategies.md)
- **`arXiv 2606.16223 did:crdt`** — HLC triple (physical_ms, counter, node_id derived from public key), product of CRDTs is CRDT, CALM theorem [https://arxiv.org/html/2606.16223](https://arxiv.org/html/2606.16223)
- **`crates.io redb 4.2.0` + `docs.rs redb 3.1.0`** — copy-on-write B+-trees, ACID MVCC, crash-safe, stable file format since 1.0, benchmarks vs sled/lmdb [https://crates.io/crates/redb](https://crates.io/crates/redb) ; [https://docs.rs/crate/redb/3.1.0](https://docs.rs/crate/redb/3.1.0)
- **`docs.rs sled 0.34.7`** — lock-free Bw-tree, `Tree::transaction`, `apply_batch`, `flush/flush_async`, beta warning ("if reliability is your primary constraint, use SQLite. sled is beta"), unstable format before 1.0 [https://docs.rs/crate/sled/0.34.7](https://docs.rs/crate/sled/0.34.7)
- **`libp2p.io releases 2025-06-28 rust-libp2p v0.56`** — umbrella bundling gossipsub 0.49 + kad 0.48 [https://libp2p.io/releases/2025-06-28-rust-libp2p/](https://libp2p.io/releases/2025-06-28-rust-libp2p/)
- **ADRs 0001/0002/0004/0006** — Ed25519 PeerId + `b"/ai-bank/1/"` domain, libp2p swarm composition, FAL-2 fixed-supply + contained blast radius + evaluation harness (`cargo test`, Sybil N=50, dust flood) — local files `docs/adr/0001-node-identity.md:1`, `docs/adr/0002-communication-protocol.md:1`, `docs/adr/0004-safety-risk-levels.md:1`, `docs/adr/0006-safety-evaluation-framework.md:1`
- **Wayfinder map #1 + tickets #10/#12** — data model deferred to #10, supply allocation to #12 — `gh issue view 10/12`

---

*Prepared for wayfinder map #1. Next step: Await #10 (data model) to finalise `Transfer`/`SignedBatch` canonical types and `DUST_THRESHOLD`/`MAX_AMOUNT` constants, then draft ADR 0007/0008 (ledger replication + storage) — gossipsub mesh + request-response sync + redb log + deterministic longest-valid-history fork-choice + `b"/ai-bank/1/"` domain-separated sigs, with Kademlia reserved for peer routing only.*

# Shared Registry: Kademlia DHT for Peer Discovery

**Wayfinder Research Ticket #8 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**
**Branch:** `research/shared-registry` | **Date:** 2026-09-03 | **Author:** Muse Spark (research subagent)
**Status:** Research complete — decision-ready
**Depends on:** ADR 0001 (Ed25519 `PeerId`), ADR 0002 (libp2p swarm)

---

## TL;DR for Decision-Maker

| Option | What it is | Verdict for AI Bank MVP |
|---|---|---|
| **A: libp2p Kademlia DHT (`libp2p-kad` + `MemoryStore`, `Mode::Server`, `k=20`, `put_record`/`get_record` + `start_providing`/`get_providers`, `identify → add_address`)** | Decentralised hash table replicated to `k` closest peers, no hosted server, peer routing + content routing, expiry/republication built-in. | **Recommended for shared registry.** Directly satisfies ADR 0001/0002, aligns with existing swarm, gives peer routing without a CA. |
| **B: Rendezvous (`libp2p-rendezvous`, namespaced `REGISTER`/`DISCOVER` with `SignedPeerRecord`, federated daemons + cookie pagination)** | Lightweight discovery via known rendezvous points; any node can be a point but clients must know its address. | **Useful as complement for bootstrap/relay discovery, not replacement.** Federated, not fully decentralised; introduces rendezvous SPOF if only one daemon [Source: libp2p specs rendezvous README — federated vs DHT/gossipsub decentralised](https://github.com/libp2p/specs/blob/master/rendezvous/README.md) |
| **C: Pure gossip / `gossipsub` only** | Flood or mesh gossip of `PeerId→addrs`; no deterministic lookup. | **Reject for registry.** No `FIND_NODE` guarantee, duplicates, unbounded fan-out; keep `gossipsub` for ledger propagation per ADR 0002. |

**Bottom line:** Ship registry on **Kademlia DHT (A)**, wire `identify::Event::Received → kad.add_address` (mandatory in `rust-libp2p`, not auto-wired), feed `mdns::Event::Discovered → kad.add_address` for LAN, bootstrap via a well-known `/dnsaddr/bootstrap.libp2p.io` or self-hosted `/ip4/.../p2p/<PeerId>` — not a central CA. Keep rendezvous (`B`) as an optional Phase-2 accelerator for relay/CID discovery. Record shape for peer routing is `PeerId` proximity, not an explicit `PeerId→Multiaddr` value record — addresses live in k-bucket entries; for signed alias hints store `Record{key: /ai-bank/peer/<PeerId>, value: CBOR(SignedPeerRecord)}` with `StoreInserts::FilterBoth` verification.

---

## 1. What Kademlia Does for AI Bank

- **Two DHT roles in one primitive** per libp2p DHT guide: (1) **Peer routing** — given `PeerId`, find its `Multiaddr`s via `FIND_NODE` over XOR distance; (2) **Content routing** — given a key, find providers via `GET_PROVIDERS` [Source: `libp2p.io` — The DHT, peer vs content routing](https://libp2p.io/guides/dht/)
- **Kademlia core:** 256-bit SHA-256 keyspace, XOR metric, `k`-buckets per prefix length `0..255`, iterative lookup contacting `α=3` closest peers in parallel, collecting `k=20` closest overall [Source: `libp2p.io` — Kademlia DHT bucket structure, `α=3`](https://libp2p.io/docs/kademlia-dht/)
- **AI Bank mapping:**
  - **Peer discovery:** `get_closest_peers(peer_id)` walks to the `k` closest nodes to `sha256(PeerId)`; returned `KadPeer` entries carry `Multiaddr`s. New nodes also learn peers via bucket `RoutingUpdated` events after `bootstrap`.
  - **Registry as value:** `put_record(Record{key: /ai-bank/registry/<name_or_peer>, value, publisher, expires}, Quorum)` replicates to `k` closest nodes to `sha256(key)` [Source: `Behaviour::put_record` stores locally + iterative `PutRecordPhase::GetClosestPeers`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)
  - **Registry as provider:** `start_providing(key)` + `get_providers(key)` for “who has ledger shard / who offers service X” pull-model pointers [Source: `RecordStore` doc — push-model (value records) vs pull-model (provider records)](https://docs.rs/libp2p-kad/latest/libp2p_kad/store/trait.RecordStore.html)
- **No central host:** Same trust model as ADR 0001 — PeerId self-certifies; bootstrap node is a well-known rendezvous, not an authority.

---

## 2. `MemoryStore` vs Persistent Stores

### 2.1 `MemoryStore` (default, in-memory)

```rust
use libp2p::kad::store::{MemoryStore, MemoryStoreConfig};

let store = MemoryStore::new(local_peer_id);
// or with limits:
let mut cfg = MemoryStoreConfig::default();
cfg.set_max_records(1024);
cfg.set_max_provided_keys(1024);
cfg.set_max_providers_per_key(20);
let store = MemoryStore::with_config(local_peer_id, cfg);
```

- **What it is:** `impl RecordStore` held entirely in RAM — `HashMap<Key, Record>` + provider multimap, no disk I/O [Source: `MemoryStore` struct doc — `MemoryStore in libp2p::kad::store`](https://docs.rs/libp2p/latest/libp2p/kad/store/struct.MemoryStore.html)
- **Lifetime:** Dropped on restart. Records survive only until process exit, then need re-replication from surviving `k` replicas + original publisher’s re-publication.
- **Limits:** Bounded by `MemoryStoreConfig` (`max_records`, `max_provided_keys`, `max_providers_per_key`) to cap RAM on small devices [Source: `MemoryStore::with_config`, `MemoryStoreConfig` fields — `docs.rs/libp2p-kad/...MemoryStoreConfig`](https://docs.rs/libp2p/latest/libp2p/kad/store/struct.MemoryStoreConfig.html)
- **MVP verdict:** **Use `MemoryStore` for MVP** — aligns with “no cloud bills / runs on user hardware” where persistence is the operator’s responsibility (same as `identity.key` per ADR 0001). Document that registry is best-effort ephemeral; ledger durability comes from gossip + local `sled`/`redb` log, not DHT alone.

### 2.2 Persistent `RecordStore` (Phase-2+)

- **Trait to implement:** `trait RecordStore { put/get/remove/records + add_provider/providers/provided/remove_provider }` [Source: `RecordStore` trait doc — two record families](https://docs.rs/libp2p/latest/libp2p/kad/store/trait.RecordStore.html)
- **Options:** Wrap `sled`, `redb`, `rocksdb`, or SQLite; example pattern is `struct PersistentStore { db: sled::Db, memory: MemoryStore }` delegating `RecordStore` calls to serialized `Record{key, value, publisher, expires}` with `bincode`/`CBOR`. No official persistent impl ships in `libp2p-kad`; users bring their own.
- **When to add:** Only after measuring that churn causes unacceptable re-lookup latency (node restarts lose `k` replicas and must wait `republication_interval` ~24h to re-converge). Keep MVP simple; persistent store adds crash-consistency bugs for little gain when `k=20` already masks single-node loss.

### 2.3 Common pitfall

- Storing a `Record` locally (`store.put`) does **not** automatically re-publish after expiry — publication job does. Passing `Record{expires: None}` means “no local expiry, but replicated with global `record_ttl` (36h default)” [Source: `Behaviour::put_record` doc — `expires None` does not expire locally but replicated with `record_ttl`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs#L1300)

---

## 3. `Mode::Server` vs `Mode::Client`

|  | `Mode::Server` | `Mode::Client` |
|---|---|---|
| **Responds to DHT queries (`FIND_NODE`, `GET_VALUE`, etc.)?** | Yes — stores records, serves `k` closest peers | No — can query but never queried |
| **Included in peers' routing tables?** | Yes (only servers populate k-buckets) | No |
| **Advertises protocol?** | Advertises `/ipfs/kad/1.0.0` via `identify` | Does not advertise, refuses inbound kad streams |
| **Who should use it** | Publicly routable nodes — datacenter, relay-capable, stable | Behind NAT/firewall, intermittent, low RAM/CPU [Source: `specs/kad-dht/README.md` — client vs server mode, routable vs NAT](https://github.com/libp2p/specs/blob/master/kad-dht/README.md) |

### 3.1 `rust-libp2p` specifics

- **Default in `libp2p-kad` ≥0.40 (2023):** Starts in `Client` mode when no external address is known; auto-switches to `Server` once `Swarm::add_external_address` or `AutoNAT` reports reachability [Source: `protocols/kad/CHANGELOG.md` — PR #3877 auto-mode, PR #4132 explicit `Mode::{Client,Server}`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md)
- **Explicit override:**

```rust
use libp2p::kad::{Behaviour, Mode};

let mut kad = Behaviour::new(local_peer_id, store);
kad.set_mode(Some(Mode::Server)); // or Some(Mode::Client)
// getter:
assert_eq!(kad.mode(), Mode::Server);
```

  Getter added in `0.47.0` [Source: `CHANGELOG.md 0.47.0` — PR #5573](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md)

- **AI Bank rule:** Bootstrap/relay nodes **must** be `Server`. User laptops behind NAT run `Client` by default (via `AutoNAT`), but call `set_mode(Some(Mode::Server))` when `AutoNAT` reports `Public` or an external `/ip4/...` is configured — otherwise they pollute DHT with undialable entries (routing table only accepts `Server` nodes [Source: `specs/kad-dht/README.md` — only servers added to routing tables](https://github.com/libp2p/specs/blob/master/kad-dht/README.md))

---

## 4. Replication `k=20`, Quorum, and Lookup Parallelism

- **`K_VALUE = 20`** — the `k` parameter: bucket size + default replication factor; all nodes must agree; configurable replication factor should be `≤ K_VALUE` [Source: `protocols/kad/src/lib.rs` — `pub const K_VALUE: NonZeroUsize = 20`, replication-factor comment](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/lib.rs)

```rust
use libp2p::kad;

assert_eq!(kad::K_VALUE.get(), 20);
let cfg = kad::Config::new(protocol);
cfg.set_replication_factor(NonZeroUsize::new(20).unwrap());
```

- **Replication factor vs bucket size:**
  - `K_VALUE` caps bucket length (routing table invariant).
  - `replication_factor` drives `put_record` — how many closest peers receive the record, and how many `k` closest peers a query contacts (`α` fan-out, then iterate) [Source: `behaviour.rs` — `QueryPool` replication factor used in `put_record` quorum eval](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)

- **Quorum (`Quorum::One | N | All`)** — minimum distinct nodes that must acknowledge `put_record`/`get_record` for success; evaluated against `replication_factor` [Source: `libp2p::kad::Quorum` — quorum eval doc](https://docs.rs/libp2p/latest/libp2p/kad/enum.Quorum.html)

```rust
use libp2p::kad::{Quorum, Record, record::Key};
use std::num::NonZeroUsize;

kad.put_record(record, Quorum::One)?;                 // succeed after 1 ack
kad.put_record(record, Quorum::N(NonZeroUsize::new(3).unwrap()))?; // 3 acks
kad.get_record(&Key::new(&"my-key"), Quorum::One);
```

- **Parallelism `α = 3`** — default iterative lookup contacts 3 closest peers in parallel per hop [Source: `libp2p.io` DHT — `α (alpha) typically 3`](https://libp2p.io/guides/dht/)

- **Security variant:** `Config::disjoint_query_paths(true)` — `S/Kademlia` disjoint paths, multiplies parallelism for Sybil resistance at bandwidth cost [Source: `behaviour.rs Config::disjoint_query_paths` — S/Kademlia comment](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)

---

## 5. Core Operations: `put_record` / `get_record` / Providers

### 5.1 Value Records (`put_record` / `get_record`)

```rust
use libp2p::kad::{Quorum, Record, record::Key};
use web_time::Instant;
use std::time::Duration;

let key = Key::new(&"/ai-bank/registry/alice");
let record = Record {
    key: key.clone(),
    value: br#"{"peer":"12D3KooW...","addrs":["/ip4/..."]}"#.to_vec(),
    publisher: None,                // Behaviour fills local PeerId
    expires: Some(Instant::now() + Duration::from_secs(36*3600)), // or None
};
let qid = kad.put_record(record, Quorum::One)?;

kad.get_record(&key, Quorum::One);

// poll events:
// Event::OutboundQueryProgressed { result: QueryResult::GetRecord(Ok(GetRecordOk{records, ..})), .. }
// Event::OutboundQueryProgressed { result: QueryResult::PutRecord(Ok(PutRecordOk{key})), .. }
```

- **Semantics:** `put_record` stores locally **and** publishes to `k` closest nodes to `Key::new(key_bytes)` via iterative `FIND_NODE` then store RPCs [Source: `Behaviour::put_record` — sets `publisher`, applies `record_ttl`, enters `PutRecordPhase::GetClosestPeers`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)
- **Write-back cache:** `put_record_to` — selective store to specific peers without local store; used internally for `Caching::Enabled` to heal replicas that missed a value [Source: `Behaviour::put_record_to` doc — “caching a found record at closest node that did not return it”](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)

### 5.2 Provider Records (`start_providing` / `get_providers`)

```rust
let key = Key::new(&"/ai-bank/ledger/shard-7");
kad.start_providing(key.clone())?; // advertises local PeerId
kad.get_providers(key.clone());
// Events:
// QueryResult::StartProviding(Ok(AddProviderOk{key}))
// QueryResult::GetProviders(Ok(GetProvidersOk{providers: Vec<PeerId>, ..}))
```

- **Difference:** Provider record only stores `ProviderRecord{key, provider: PeerId, expires, addresses}` — a pointer — not the value itself; consumers must dial provider to fetch actual ledger bytes [Source: `RecordStore` — “provider records are mere pointers”](https://docs.rs/libp2p/latest/libp2p/kad/store/trait.RecordStore.html) + `ProviderRecord` struct fields](https://docs.rs/libp2p/latest/libp2p/kad/struct.ProviderRecord.html)
- **Store budget:** `add_provider` only keeps `replication_factor` providers per key, preferring those closest to the key [Source: `RecordStore::add_provider` doc](https://docs.rs/libp2p/latest/libp2p/kad/store/trait.RecordStore.html)

### 5.3 Peer Routing (`get_closest_peers`)

```rust
let target: PeerId = "12D3KooW...".parse()?;
kad.get_closest_peers(target);
// -> QueryResult::GetClosestPeers(Ok(GetClosestPeersOk{key, peers: Vec<PeerId>>})) 
//    or Err(GetClosestPeersError::Timeout{key, peers})
```

Core building block for both above; also used by `bootstrap`.

---

## 6. Wiring `identify` → `kad.add_address` (Mandatory Hook)

`rust-libp2p` deliberately does **not** auto-wire Identify into Kademlia [Source: `protocols/kad/src/lib.rs` — “Peer Discovery with Identify … must be manually hooked up … through calls to `Behaviour::add_address`”](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/lib.rs)

```rust
use libp2p::{kad, identify, swarm::SwarmEvent};

// in main event loop:
match swarm.next().await {
    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(e)) => match *e {
        identify::Event::Received { peer_id, info: identify::Info { listen_addrs, protocols, .. } } => {
            // only feed kad-speaking peers — avoids polluting table with non-kad peers
            if protocols.iter().any(|p| *p == kad::PROTOCOL_NAME) {
                for addr in listen_addrs {
                    let _ = swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
            }
        }
        _ => {}
    },
    // optional symmetric direction: feed inbound kad RoutingUpdated into address book
    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::RoutingUpdated{ peer, addresses, .. })) => {
        // addresses already in k-buckets; nothing else required for kad, but useful for relay/gossipsub
    }
    _ => {}
}
```

- **Two purposes of `add_address`** — (1) seed routing table from bootstrap peer; (2) learn dialable address of an inbound connection before it can be admitted to a k-bucket [Source: `Behaviour::add_address` — two purposes + `Event::RoutingUpdated`](https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html)
- **Filter by protocol** — check `protocols.contains(kad::PROTOCOL_NAME)` (typically `/ipfs/kad/1.0.0` or custom `/ai-bank/kad/1.0.0`); stale pattern `p.as_bytes() == kad::protocol::DEFAULT_PROTO_NAME` also seen in examples [Source: `discovery-identify-kademlia` example — `protocols.iter().any(|p| p.as_bytes() == kad::protocol::DEFAULT_PROTO_NAME)`](https://github.com/gcp-development/ipfs-private-network/blob/main/discovery-identify-kademlia/src/main.rs)
- **Bootstrapping still explicit:** `add_address` only inserts into routing table; caller must still `kad.bootstrap()?` to start lookups; periodic bootstrap runs automatically every 5 min by default [Source: `Behaviour::bootstrap` — requires non-empty table, auto periodic via `set_periodic_bootstrap_interval`](https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html)
- **Official runnable example with both protocols:** `examples/ipfs-kad` uses `MemoryStore` + `add_address("/dnsaddr/bootstrap.libp2p.io")` [Source: `examples/ipfs-kad/src/main.rs` — BOOTNODES + `SwarmBuilder::with_tokio`](https://github.com/libp2p/rust-libp2p/blob/master/examples/ipfs-kad/src/main.rs) ; `distributed-key-value-store.rs` shows `mDNS`-fed variant [Source: `examples/distributed-key-value-store.rs` — `mdns::MdnsEvent::Discovered → kademlia.add_address`](https://github.com/libp2p/rust-libp2p/blob/e437c009dc80777300e56c0c06d73ff14e5449a1/examples/distributed-key-value-store.rs)

**Failure mode if omitted:** DHT never grows beyond bootstrap peers; inbound connections remain un-routable; `get_closest_peers` returns only bootstrap [Source: `Discussion #2673` — “DHT isn't being shared among peers without Identify hook”](https://github.com/libp2p/rust-libp2p/discussions/2673) and [`lib.rs` Important Discrepancies quote above](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/lib.rs)

---

## 7. Expiry, Caching, Conflict Resolution

### 7.1 TTLs and Intervals (default values)

| Parameter | Default | Applies to | Note |
|---|---|---|---|
| `record_ttl` | 36 hours | value records | `None` = never expire [Source: `Config::set_record_ttl`](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html) |
| `replication_interval` | 1 hour | value records | re-replicate to new closest peers on churn; does not extend TTL [same](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html) |
| `publication_interval` | 24 hours | value records | original publisher re-publishes, extending TTL; `None` = never re-publish [same](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html) |
| `provider_record_ttl` | 48 hours | provider records | `None` = never expire [Source: `Config::set_provider_record_ttl`](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html) |
| `provider_publication_interval` | 22 hours | provider records | must be << provider TTL [Source: `Config::set_provider_publication_interval`](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html) |
| `periodic_bootstrap_interval` | 5 minutes | routing table health | auto self-lookup + bucket refresh [Source: `Behaviour::bootstrap` periodic note + `Config::set_periodic_bootstrap_interval`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs) |

Invariant: `replication_interval << publication_interval << record_ttl`; likewise `provider_publication_interval << provider_record_ttl`.

`Record { expires: Option<Instant> }` and `ProviderRecord { expires }` are checked via `is_expired(now)`; lazy cleanup of expired providers was added in `0.47.0` [Source: `Record` struct, `ProviderRecord` struct](https://docs.rs/libp2p/latest/libp2p/kad/struct.Record.html) and [Source: `CHANGELOG.md 0.47+` — PR #5980 lazy cleanup](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md)

### 7.2 Caching (`Caching::Enabled { max_peers: 1 }` default)

After a successful `get_record`, the `max_peers` closest peers that did **not** return a record are returned in `GetRecordOk::FinishedWithNoAdditionalRecord` and should be explicitly written back via `put_record_to` — the “write-back cache” that heals replicas missed during replication gaps [Source: `behaviour.rs Caching` enum + `set_caching` — default `Enabled{max_peers:1}`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)

```rust
use libp2p::kad::{Caching, QueryResult};

// after QueryResult::GetRecord(Ok(ok)):
if let Some(missing) = ok.cache_candidates.into_iter().next() {
    kad.put_record_to(chosen_record, missing.into_iter(), Quorum::One);
}
```

Disable with `Caching::Disabled` if bandwidth is tighter than hit-rate.

### 7.3 Conflict Resolution & Filtering (`StoreInserts`)

- **No CRDT / vector clock in vanilla Kademlia.** Last write (largest `seq` or newest `publisher` re-put) wins per key; concurrent puts to same key race and the `k` replicas converge on whichever arrived last at each replica (no ordering guarantee).
- **For AI Bank aliases/records:** Adopt the `libp2p-identity` `SignedEnvelope` / `PeerRecord` pattern: `value = CBOR{ seq: u64, peer: PeerId, addrs: Vec<Multiaddr>, sig: Ed25519(sig over seq||addrs)}` with monotonic `seq`; receivers reject `seq <= stored.seq`. Use `StoreInserts::FilterBoth` to gate puts through verification:

```rust
use libp2p::kad::{Config, StoreInserts, Event, InboundRequest};

let mut cfg = Config::new(protocol);
cfg.set_record_filtering(StoreInserts::FilterBoth);
let mut kad = Behaviour::with_config(local_peer_id, store, cfg);

// poll:
if let Event::InboundRequest{ request: InboundRequest::PutRecord{ source, record: Some(rec), .. }, .. } = event {
    if verify_signed_record(&rec) { kad.store_mut().put(rec).ok(); }
}
```

  [Source: `StoreInserts` — `Unfiltered` auto-stores vs `FilterBoth` emits `InboundRequest::PutRecord/AddProvider`](https://docs.rs/libp2p/latest/libp2p/kad/enum.StoreInserts.html) and [`behaviour.rs StoreInserts` doc](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)

- **Provider vs value choice for alias hints:** If aliases are hints (petname overlay per ADR 0001), prefer value records (`put_record`) with signed payload over unauthenticated provider pointers; provider records carry no signature field and are truncatable to closest providers per key [Source: `RecordStore::add_provider` — keeps `replication_factor` closest providers](https://docs.rs/libp2p/latest/libp2p/kad/store/trait.RecordStore.html)

---

## 8. Bootstrap Peers & How `mDNS` Feeds Kademlia

### 8.1 Bootstrap

```rust
// well-known bootstrap — not a CA, just an entry point
for peer in &["QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN"] {
    kad.add_address(&peer.parse()?, "/dnsaddr/bootstrap.libp2p.io".parse()?);
}
kad.bootstrap()?; // -> QueryId; emits QueryResult::Bootstrap per bucket

// periodic via config (default 5m auto)
cfg.set_periodic_bootstrap_interval(Some(Duration::from_secs(300)));
// or manual timer:
loop { tokio::time::sleep(Duration::from_secs(300)).await; let _ = kad.bootstrap(); }
```

- **Requires non-empty routing table** — `bootstrap()` returns `Err(NoKnownPeers)` if no peer was `add_address`ed [Source: `Behaviour::bootstrap` — Err on empty table + self-lookup description](https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html)
- **Multi-step:** Self-lookup for own bucket, then random keys for farther buckets [Source: `Behaviour::bootstrap` — “all buckets farther from closest neighbour are refreshed”](https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html)
- **What bootstrap stores:** Nothing — bootstrap peers are routing-table seeds, not ledger stores; any `Server` peer can seed.

### 8.2 `mDNS` → Kademlia

For LAN (no cloud bills, same Wi-Fi / lab), `mdns` is zero-config discovery that should **feed** kademlia rather than replace it:

```rust
use libp2p::{mdns, kad, swarm::SwarmEvent};

// NetworkBehaviour { kademlia: kad::Behaviour, mdns: mdns::tokio::Behaviour, identify: identify::Behaviour }

// on mdns discovery:
SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
    for (peer_id, addr) in list {
        kad.add_address(&peer_id, addr);
    }
}
// also keep identify hook from §6 — mDNS provides LAN addrs, identify provides public addrs
```

- **Pattern validated in official example** `distributed-key-value-store.rs` wires `MdnsEvent::Discovered → kademlia.add_address` [Source: `examples/distributed-key-value-store.rs`](https://github.com/libp2p/rust-libp2p/blob/e437c009dc80777300e56c0c06d73ff14e5449a1/examples/distributed-key-value-store.rs)
- **Also seen in community tutorial** `book.univrs.io` Step 3 — mDNS + Kademlia together [Source: `book.univrs.io` — Automated Discovery with Kademlia DHT](https://book.univrs.io/docs/rust-orchestration/rust_peer-to_peer_plan)

**AI Bank recommendation:** Enable `mdns` unconditionally on `cfg(target_os != "ios")` for LAN parties / CI; disable on iOS where multicast is unavailable. Never rely solely on mDNS — it does not cross subnets.

---

## 9. Rendezvous as an Alternative (and Why DHT Wins for MVP)

| Dimension | Kademlia DHT | Rendezvous (`libp2p-rendezvous`) | Pure `gossipsub` |
|---|---|---|---|
| **Topology** | Fully decentralised, `k=20` replicas, XOR routing | Federated — clients register at known rendezvous points | Mesh flood, no routing table |
| **Lookup guarantee** | `FIND_NODE` / `GET_VALUE` iterative success if `k` replicas live | Only if rendezvous point reachable | No deterministic lookup |
| **Namespace** | Opaque 256-bit keys (`/ai-bank/...` prefixes) | Explicit namespaces (`"ai-bank/registry"`) + cookie pagination [Source: `specs/rendezvous/README.md` — `REGISTER` namespaces, cookie](https://github.com/libp2p/specs/blob/master/rendezvous/README.md) | Topics (`gossipsub::IdentTopic`) |
| **Security** | `SignedEnvelope` + `StoreInserts::FilterBoth` verification | Registration self-signs `SignedPeerRecord`, point validates | Peer-score + message `Strict` validation |
| **SPOF** | None — any `Server` is a DHT member | Single rendezvous point = SPOF; federation needs fleet [Source: libp2p docs — rendezvous is federated not decentralised, single point of failure](https://libp2p.io/docs/rendezvous/) | Bootstrap mesh |
| **When to use** | Peer registry, `PeerId → addrs`, signed alias hints, provider pointers | Bootstrap discovery of relays / circuit reservations, app-specific service lookup | Ledger / transfer propagation after discovery |
| **Crate** | `libp2p-kad` | `libp2p-rendezvous` (`rendezvous::client::Behaviour`, `rendezvous::server::Behaviour`) | `libp2p-gossipsub` |

**Detailed rendezvous flow (per spec):**

1. Client sends `REGISTER(namespace="ai-bank/registry", signedPeerRecord, ttl)` to point.
2. Point stores with TTL (recommended max per point config, defend against DoS — “maximum registrations” per spec) [Source: `specs/rendezvous/README.md` — REGISTER + recommended max/TTL + pagination](https://github.com/libp2p/specs/blob/master/rendezvous/README.md)
3. Discoverer queries `DISCOVER(namespace, cookie, limit)`; point returns matching `SignedPeerRecord`s + next cookie.

**Trade-off for AI Bank:** Rendezvous is simpler to reason about (central index) and has cookie-pagination for large namespaces, but contradicts the “no hosted infra” constraint unless the community self-hosts a fleet. Use rendezvous **later** for relay discovery (Phase-1 `autonat+relay+dcutr` per ADR 0002) via a well-known `/ai-bank/rendezvous` namespace, not as the primary registry.

---

## 10. Rust Crates & Versions — How to Verify

### 10.1 Crate Map (current at 2026-09, per `crates.io` + `rust-libp2p`)

| Crate | Latest | MSRV | Needed for registry | Source |
|---|---|---|---|---|
| `libp2p-kad` | **0.48.0** (2025-06-27) — prior `0.47.0` (2025-01-14) yanked on subcrate churn | 1.83.0 | DHT core | [crates.io `libp2p-kad` — 0.48.0, MSRV, history](https://crates.io/crates/libp2p-kad/0.48.0) ; changelog [Source: `CHANGELOG.md 0.48.0/0.47.0`](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md) |
| `libp2p` umbrella | **0.56.0** (2025-06-28) — bundles kad `0.48`, mdns `0.46`, identify `0.46`, swarm `0.46` etc. | 1.83.0 | Single version pin | [Source: `libp2p.io/releases/2025-06-28-rust-libp2p` — v0.56 announcement](https://libp2p.io/releases/2025-06-28-rust-libp2p/) + `crates.io libp2p` |
| `libp2p-identify` | 0.46.x (in umbrella 0.56) | 1.83.0 | `identify → add_address` hook | Same umbrella release notes |
| `libp2p-mdns` | 0.46.x (tokio: `mdns::tokio::Behaviour` — `Mdns` deprecated 0.43+) | 1.83.0 | LAN discovery | Same + example wiring above |
| `libp2p-rendezvous` | 0.15.x (in umbrella 0.56) | 1.83.0 | Optional Phase-2 | Same umbrella source |

**Feature flags on `libp2p`:** `features = ["tokio", "tcp", "quic", "dns", "noise", "yamux", "identify", "kad", "mdns", "gossipsub", "relay", "dcutr", "autonat"]` per ADR 0002. Avoid enabling `full` in production (pulls wasm transports). Exact flag list lives at [`docs.rs/crate/libp2p/0.56.0/features`](https://docs.rs/crate/libp2p/0.56/features) table referenced in `research/communication-protocol`.

### 10.2 Verification Commands (copy-paste)

```bash
# 1) crates.io metadata (no clone needed)
cargo search libp2p-kad --limit 3
cargo info libp2p-kad@0.48.0   # MSRV + deps + repository

# 2) local pin (Cargo.toml)
# [dependencies]
# libp2p = { version = "0.56.0", features = ["tokio","tcp","quic","dns","noise","yamux","identify","kad","mdns"] }
cargo tree | grep -E "libp2p-kad|libp2p "
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="libp2p-kad") | {version, rust_version}'

# 3) docs.rs as source of truth per-field
# https://docs.rs/libp2p-kad/0.48.0/libp2p_kad/store/struct.MemoryStore.html
# https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html  # umbrella re-export
# https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md  # PR provenance (e.g., 3877 auto-mode, 4132 explicit Mode, 5573 mode getter, 5980 lazy expiry)

# 4) spec as authority for client vs server semantics
# https://github.com/libp2p/specs/blob/master/kad-dht/README.md#client-and-server-mode
```

Pin MVP to **`libp2p 0.56 + libp2p-kad 0.48`** (MSRV 1.83) for longest support window; `0.54/0.55` remain viable at MSRV 1.75/1.83 but miss `0.48`’s `substreams_timeout` rename and lazy provider cleanup [Source: `CHANGELOG 0.48.0` PR #6015/#6076](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md).

---

## 11. Recommendations for AI Bank Registry on This Stack

1. **Default store `MemoryStore` with caps** — `MemoryStore::with_config(local_peer_id, MemoryStoreConfig{max_records:2048, max_providers_per_key:64})`; document ephemeral nature; add persistent `RecordStore` only after churn metrics justify it (§2).
2. **Explicit `Mode` handling** — Bootstrap nodes hard-set `Mode::Server`; clients start as `Client` and flip to `Server` when `AutoNAT` reports `Public` or `Swarm::external_addresses` non-empty (§3).
3. **Record shape for peer alias / addr hints:**

```rust
#[derive(Serialize, Deserialize)]
struct PeerRecord Hint {
    seq: u64,                      // monotonic
    peer: PeerId,
    addrs: Vec<Multiaddr>,
    alias: Option<String>,         // petname hint, never authority
    sig: Vec<u8>,                  // Ed25519(sign(domain b"/ai-bank/1/peer-record:" || seq || addrs))
}
// DHT key: record::Key::new(&format!("/ai-bank/peer/{}", peer.to_base58()))
// put_record(Hint{..}, Quorum::One) + verify via StoreInserts::FilterBoth (§7.3)
```

4. **Wire both discoverers into kad** — `identify::Event::Received` → `kad.add_address` (filtered by `kad::PROTOCOL_NAME`) + `mdns::Event::Discovered` → `kad.add_address` (§6, §8.2). This is the only way the routing table grows beyond bootstrap in `rust-libp2p`.
5. **Lifecycle wiring** — On `NewListenAddr` also `swarm.add_external_address(addr.clone())` so auto-mode flips; on a 5-min timer call `kad.bootstrap().ok()`; on `GetRecord` success, optionally `put_record_to` the `cache_candidates` for write-back (§7.2).
6. **Expiry tuning for MVP** — Keep defaults (`record_ttl 36h`, `publication 24h`, `replication 1h`, `provider ttl 48h`, `provider publication 22h`) — they satisfy `replication << publication << ttl` invariant without constant republishing on laptops that sleep. Consider shorter TTL (12h/6h/1h) only for alias hints where freshness beats durability.
7. **Rendezvous later** — Add `libp2p-rendezvous` client in Phase-2 for “find relay near me” (`namespace="ai-bank/relay"`), federated across two community-run points; keep DHT as canonical registry so a single rendezvous SPOF does not block peer routing (§9).
8. **Verification gate** — CI runs `cargo tree | grep libp2p-kad`, `cargo info`, and checks `docs.rs` links in this doc still resolve; update ADR 0002→ADR 0007 (registry) with the `BankBehaviour { kad, identify, mdns, gossipsub, relay, dcutr, autonat }` composition.

---

## Sources — Primary Only (Every Claim Traces Above)

- **`specs/kad-dht/README.md`** — client vs server mode, routing-table membership, replication-factor guidance [https://github.com/libp2p/specs/blob/master/kad-dht/README.md](https://github.com/libp2p/specs/blob/master/kad-dht/README.md)
- **`specs/rendezvous/README.md`** — REGISTER/DISCOVER, namespaces, cookie, `SignedPeerRecord`, recommended max/TTL, federated vs decentralised [https://github.com/libp2p/specs/blob/master/rendezvous/README.md](https://github.com/libp2p/specs/blob/master/rendezvous/README.md)
- **`rust-libp2p/protocols/kad/src/lib.rs`** — `K_VALUE=20`, “Peer Discovery with Identify must be manually hooked up” [https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/lib.rs](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/lib.rs)
- **`rust-libp2p/protocols/kad/src/behaviour.rs`** — `Config` defaults (36h/24h/1h, 48h/22h), `Caching`, `StoreInserts::FilterBoth`, `put_record`/`put_record_to` semantics, `periodic_bootstrap_interval=5m` [https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/src/behaviour.rs)
- **`rust-libp2p/protocols/kad/CHANGELOG.md`** — 0.48.0→0.47.0 diffs, PRs #3877 (auto-mode), #4132 (explicit Mode), #5573 (mode getter), #5980 (lazy expiry), #6015/#6076 (substreams_timeout) [https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md](https://github.com/libp2p/rust-libp2p/blob/master/protocols/kad/CHANGELOG.md)
- **`docs.rs/libp2p-kad` / `docs.rs/libp2p`** — `MemoryStore`/`MemoryStoreConfig`, `Behaviour::add_address`/`bootstrap`, `Quorum`, `Record`/`ProviderRecord`, `Config::set_*` TTL APIs [https://docs.rs/libp2p-kad/0.48.0](https://docs.rs/libp2p-kad/0.48.0) ; [https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html](https://docs.rs/libp2p/latest/libp2p/kad/struct.Behaviour.html) ; [https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html](https://docs.rs/libp2p/latest/libp2p/kad/struct.Config.html) ; [https://docs.rs/libp2p/latest/libp2p/kad/enum.StoreInserts.html](https://docs.rs/libp2p/latest/libp2p/kad/enum.StoreInserts.html)
- **`crates.io/crates/libp2p-kad/0.48.0`** — version/MSRV/deps, history [https://crates.io/crates/libp2p-kad/0.48.0](https://crates.io/crates/libp2p-kad/0.48.0) ; **`libp2p.io/releases/2025-06-28-rust-libp2p`** — umbrella 0.56 bundling kad 0.48 [https://libp2p.io/releases/2025-06-28-rust-libp2p/](https://libp2p.io/releases/2025-06-28-rust-libp2p/)
- **`libp2p.io/guides/dht` & `libp2p.io/docs/kademlia-dht` & `libp2p.io/docs/rendezvous`** — peer vs content routing, α=3, k=20, bucket structure, federated vs decentralised [https://libp2p.io/guides/dht/](https://libp2p.io/guides/dht/) ; [https://libp2p.io/docs/kademlia-dht/](https://libp2p.io/docs/kademlia-dht/) ; [https://libp2p.io/docs/rendezvous/](https://libp2p.io/docs/rendezvous/)
- **`examples/ipfs-kad/src/main.rs` & `examples/distributed-key-value-store.rs` & `discovery-identify-kademlia` example** — runnable `MemoryStore::new + add_address(dnsaddr/bootstrap)` and `MdnsEvent::Discovered → kad.add_address` patterns [https://github.com/libp2p/rust-libp2p/blob/master/examples/ipfs-kad/src/main.rs](https://github.com/libp2p/rust-libp2p/blob/master/examples/ipfs-kad/src/main.rs) ; [https://github.com/libp2p/rust-libp2p/blob/e437c009dc80777300e56c0c06d73ff14e5449a1/examples/distributed-key-value-store.rs](https://github.com/libp2p/rust-libp2p/blob/e437c009dc80777300e56c0c06d73ff14e5449a1/examples/distributed-key-value-store.rs)
- **Discussions #2673/#5472/#4702** — why Identify hook is required for DHT growth beyond bootstrap, client-mode pitfall [https://github.com/libp2p/rust-libp2p/discussions/2673](https://github.com/libp2p/rust-libp2p/discussions/2673) ; [https://github.com/libp2p/rust-libp2p/discussions/5472](https://github.com/libp2p/rust-libp2p/discussions/5472)

---

*Prepared for wayfinder map #1. Next step: Draft ADR 0007 (registry) — Kademlia value records for signed `PeerRecord` hints with `MemoryStore`, `Mode::Server` bootstrap, `identify+mdns→kad` wiring, default TTLs, `FilterBoth` signed-write gate, and rendezvous as optional relay-discovery namespace.*

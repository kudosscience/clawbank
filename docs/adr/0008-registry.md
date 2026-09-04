# ADR 0008: Shared registry is Kademlia value records for signed PeerRecord hints

The shared registry is Kademlia DHT value records on the libp2p swarm (ADR 0002): `MemoryStore` (ephemeral) with `Mode::Server` on bootstrap/relay nodes (auto-mode otherwise), manual `identify` + `mdns → kad.add_address` wiring, default TTLs (`record_ttl 36h` / `publication 24h` / `replication 1h` / `provider 48h/22h` / `bootstrap 5m`), `StoreInserts::FilterBoth` signed-write gate for `SignedPeerRecord` alias hints under `/ai-bank/peer/<PeerId>`, and optional rendezvous namespace for relay discovery — see `research/shared-registry` decision record.

## Status

Accepted — implements wayfinder ticket [#8 Shared registry: Kademlia DHT for peer discovery](https://github.com/kudosscience/clawbank/issues/8) → `research/shared-registry` (`docs/research/shared-registry.md:1` on `research/shared-registry`). Depends on ADRs 0001/0002, aligns with ADR 0007 data-model `AccountId=PeerId` and redb persistence.

*Note: ticket text requested 0007 for this ADR; 0007 is already `docs/adr/0007-data-model.md` (PeerId=AccountId), so this records as 0008 to keep numbering monotonic.*

## Context

Nodes run on user hardware behind NAT with no cloud bill. Peer discovery must be decentralised, self-certified (no CA), and forkable. Bootstrap is a well-known `/dnsaddr` or `/ip4/.../p2p/<PeerId>` — not an authority. Kademlia provides both peer routing (`get_closest_peers` over XOR/SHA-256, `k=20`, `α=3`) and content routing (`put_record`/`get_record` replicating to `k` closest). The registry must store `PeerId→Multiaddr` for routing and signed alias hints for display (petname overlay ADR 0001).

## Considered Options

- **Kademlia DHT value + provider records (chosen)** — `libp2p-kad 0.48.0` (MSRV 1.83) `MemoryStore::new/with_config`, `K_VALUE=20`, `Quorum::One/N/All`, `α=3`, periodic `replication_interval 1h`, `record_ttl 36h` / `record_publication 24h` / `provider_ttl 48h` / `provider_publication 22h` / `periodic_bootstrap 5m`; `OnConnected`→`bootstrap()` guarded on `NoKnownPeers`. Value records for `/ai-bank/peer/<PeerId>` alias hints, provider records (`start_providing`/`get_providers`) for pull-model pointers only; Caching write-back disabled for MVP. Payloads in libp2p DHT guide and `Behaviour::put_record` docs.
- **Manual wiring identify+mdns→kad (chosen, mandatory)** — `identify::Event::Received { peer_id, info: { listen_addrs, observed_addr } }` → if `protocols.contains(kad::PROTOCOL_NAME)` then `kad.add_address(&peer_id, addr)`; same for `mdns::Event::Discovered`. Rust-libp2p does not auto-wire (documented "Important Discrepancies"); `examples/ipfs-kad` + `distributed-key-value-store.rs` + discussion #2673 confirm without it routing stays at bootnode.
- **StoreInserts::FilterBoth signed-write gate (chosen)** — `InboundRequest::PutRecord` verification against domain `b"/ai-bank/1/batch:"`/`b"/ai-bank/1/peer:"` Ed25519 over canonical CBOR (`research/data-model`), publisher PeerId must verify; unsigned/invalid → `P₄` penalty + drop; gossipsub `ValidationMode::Strict` same rule. `Caching::Enabled{1}` deferred.
- **Rendezvous federated namespace (deferred as optional relay-discovery)** — `libp2p-rendezvous` `REGISTER(namespace, SignedPeerRecord)` / `DISCOVER(cookie,limit)` via federated daemons, cookie pagination; complements DHT for relay `p2p-circuit` discovery (`specs/rendezvous/README.md`) but federated SPOF, not replacement. Keep for Phase-2 relay-discovery namespace `/ai-bank/relay/1.0.0`, not primary registry.
- **Pure gossipsub flooding for registry (rejected)** — no `FIND_NODE` guarantee, unbounded fan-out; reserved for ledger propagation (ADR 0002).
- **Persistent RecordStore over sled/redb (deferred)** — custom `impl RecordStore` delegates to serialized `Record{key,value,publisher,expires}`; MVP stays `MemoryStore` (identity key already on disk per ADR 0001) until churn shows re-lookup latency, since `k=20` masks single-node loss.

## Consequences

- Bootstrap/relay nodes set `kad.set_mode(Some(Mode::Server))` (explicit, getter PR #5573); clients behind NAT stay `Client` until `AutoNAT::Public` + external address (auto-mode PR #3877/#4132), never pollute table with undialable entries.
- Record shape: provider routing is `PeerId` XOR proximity (k-bucket entries); alias hints are `Record{key: /ai-bank/peer/<PeerId>, value: CBOR(SignedPeerRecord{seq, addrs, aliases, sig}), publisher, expires}`; sequences monotonic like `SignedEnvelope`/`PeerRecord`, last-write-wins no CRDT, expiry 36h so alias is hint not authority (petname overlay remains local per ADR 0001).
- Crate pin: `libp2p 0.56` umbrella (`kad 0.48`, `identify`, `mdns`, `gossipsub 0.49`, `request-response cbor`, `noise`/`yamux`/`quic`/`dns`/`relay`/`dcutr`/`autonat` from ADR 0002) + `cbor4ii 0.3` + `schemars 1.1` + `redb 4.x` (stable since 1.0) for ledger, no new registry DB.
- Verify via `cargo tree|metadata|info` + `docs.rs` + `specs/kad-dht` (`K_VALUE`, `Quorum`, `RecordStore`).
- Open for #9: ledger replication imports alias-record verification via same `FilterBoth` without coupling fork-choice.

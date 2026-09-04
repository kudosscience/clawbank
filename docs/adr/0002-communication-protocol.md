# ADR 0002: Node↔node is libp2p swarm; agent↔node stays localhost HTTP

Node-to-node communication uses a libp2p swarm (Noise XX or TLS 1.3 + Yamux + QUIC/TCP, with identify, Kademlia, AutoNAT, relay v2, DCUtR, mDNS, gossipsub/request-response). The agent-to-node plane remains the localhost HTTP API on `127.0.0.1` (axum) decided in ADR 0003 — the two planes are transport-separated and share a single Tokio runtime and service layer — see `research/communication-protocol` decision record.

## Status

Accepted — implements wayfinder ticket [#4 Communication protocol: how nodes talk to each other](https://github.com/kudosscience/clawbank/issues/4) → `research/communication-protocol` (`docs/research/communication-protocol.md:1`). Depends on ADR 0001 (Ed25519 PeerId).

## Context

Nodes must run on users' machines with no cloud bills and work behind NAT/firewall/CGNAT. HTTP/REST, gRPC, and WebSocket alone provide no NAT traversal, no PeerId-native identity binding, and no peer discovery — requiring public IPs or paid relays. The identity decision (ADR 0001: Ed25519 → `PeerId 12D3Koo…`) makes libp2p's handshake-level verification the natural fit.

## Considered Options

- **libp2p swarm (chosen for node↔node)** — `libp2p 0.54.1` (MSRV 1.75) / `0.55-0.56` (MSRV 1.83), features `tokio`, `tcp`, `quic`, `dns`, `noise`, `yamux`, `identify`, `kad`, `autonat`, `relay`, `dcutr`, `mdns`, `gossipsub`. Noise `Noise_XX_25519_ChaChaPoly_SHA256` (or TLS extension OID `1.3.6.1.4.1.53594.1.1`) binds ephemeral X25519 to long-term identity via `identity_sig`; Yamux multiplexes. Discovery via `identify` → `kad.add_address` + `mdns` LAN + `rendezvous` later. NAT via `upnp` opportunistic + `autonat` classification + `relay v2` hop/stop with reservation vouchers (`libp2p-relay-rsvp` `0x0302`, `Limit{duration,data}`) + `dcutr` hole-punch (`Connect`/`Sync`, `RTT/2` simultaneous dial, TCP open / QUIC spray).
- **HTTP/REST+JSON (retained for agent↔node only)** — `axum 0.8` localhost; trivial for agents, no traversal/discovery/identity for P2P — rejected for inter-node.
- **gRPC `tonic` (deferred)** — typed streams, but same reachability gap; can run later *over* libp2p via `libp2p-grpc-rs` if schema pressure demands.
- **WebSocket (deferred as standalone)** — `axum ws` / `tokio-tungstenite`; viable only as additional libp2p transport (`libp2p-websocket`/`webtransport-websys`/`webrtc-websys`) for browser tier, not as node↔node protocol.

## Consequences

- One swarm per node (`SwarmBuilder::with_tokio` → TCP+QUIC → DNS → relay client → `BankBehaviour { relay_client, identify, kad, autonat, dcutr, mdns, gossipsub, ping }`); app protocols `request-response` (CBOR `TransferReq/Res`) and `gossipsub` topics `/ai-bank/transfer/1.0.0` with payload signing `b"/ai-bank/1/"`.
- `kad` is `Mode::Server` by default; must wire `identify::Event::Received` → `kad.add_address` — libp2p does not auto-wire.
- Relay is community-run and capped; any public node can be a relay (~30-line behaviour); bootstrap is a well-known `/ip4/.../p2p/<PeerId>` — not a CA.
- Symmetric NAT fallback stays relay-routed (5–15% of nodes); show `relay-only (limited)` UX.
- Service layer stays transport-blind: localhost HTTP handler and P2P handler both call `service::{balance,transfer}`.
- Phasing: Phase 0 swarm+`identify`+`ping` smoke test → Phase 1 `kad`+`autonat`+`relay`+`dcutr`+`mdns`+`gossipsub` → Phase 2 `quic`+`upnp`+`dns` etc. → Phase 3 `tonic` overlay if needed.

# Communication Protocol: How Nodes Talk to Each Other

**Wayfinder Research Ticket #4 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**
**Branch:** `research/communication-protocol` | **Date:** 2026-09-02 | **Author:** Muse Spark (research subagent)
**Status:** Research complete — decision-ready
**Dependency note:** Ticket #4 is blocked by #2 (Node identity). This research proceeds in parallel and notes #2 at each touchpoint — where #2 decision changes the analysis, it is flagged as `Depends on #2: …`. The recommended core identity from #2 is **Ed25519 keypair → libp2p PeerId (`12D3Koo…`) with local petname table**; this doc assumes that outcome but calls out fallback if #2 chose differently.

---

## TL;DR for Decision-Maker

| Option | What it is | Verdict for AI Bank |
|---|---|---|
| **A: libp2p (Swarm + Noise/TLS + Yamux + QUIC/TCP, Kademlia + identify + mDNS, relay + DCUtR)** | Modular P2P stack: transport-agnostic, encrypted, multiplexed, NAT-aware, identity-native, discovery-native. | **Recommended for node↔node (inter-node P2P).** Only option that gives NAT traversal, PeerId-identity, peer discovery and transport agility without a cloud bill. Rust `libp2p 0.54.1` (MSRV 1.75) / `0.56.0` (MSRV 1.83) is mature and Tokio-native. |
| **B: HTTP/REST+JSON (axum/http)** | Request/response over HTTP/1.1 or HTTP/2, JSON bodies, status codes. One `GET /balances/{peerId}` etc. | **Recommended for agent↔node (localhost) only — not for node↔node.** Trivial to build, universal tooling (`curl`, OpenAPI), but no NAT traversal, no identity, no streaming, manual discovery. Keep for the local control plane (ticket #3 already chose `axum` on `127.0.0.1`). |
| **C: gRPC (tonic + prost, HTTP/2 + protobuf)** | Typed unary + streaming RPC over HTTP/2, binary framing, code-gen, deadlines/metadata/trailers, bidirectional streams. | **Viable if you already love protobuf and need multiplexed streams today; rejected as primary P2P.** Still no NAT traversal, no PeerId, no discovery; adds build complexity (`tonic-build`/`proto` toolchain). Can run *over* libp2p streams via `libp2p-grpc-rs` if you want both later. |
| **D: WebSocket (axum `ws` / `tokio-tungstenite`)** | Persistent full-duplex TCP frames, client-initiated upgrade, then bidirectional messages. | **Rejected as primary node↔node.** Good for browser dashboards / server push, but clients behind NAT cannot be dialled, no identity/discovery, manual heartbeat/reconnect framing. Useful as *one* libp2p transport (`libp2p-websocket`) for browser reachability, not as a protocol. |

**Bottom line:** Keep the two planes separate. **Agent↔node = `axum` HTTP on `127.0.0.1`** (already decided in #3). **Node↔node = `libp2p` swarm** for MVP: Noise or TLS for encryption, Yamux for multiplexing, `identify`+`kad` for discovery, `autonat`+`relay v2`+`dcutr` for NAT traversal, `gossipsub` or `request-response` as app protocols for transfers/ledger. HTTP/gRPC/WebSocket solve none of "no cloud, behind NAT, who are you" — libp2p bundles all three.

---

## 1. What Node↔Node Communication Must Do (AI Bank MVP constraints)

Derived from wayfinder map #1 + #2/#3 decisions:

- **No cloud bills / no hosted infra.** Nodes run on users' existing hardware. Diagnosis must not assume a relay server you operate forever — it *may* opportunistically use a community relay, but MVP must function with zero mandatory server.
- **Users behind NAT / firewall / CGNAT.** Home routers, corporate firewalls, mobile hotspots. A protocol that requires both peers to have public IPs is deployment-blocking.
- **Identity is crypto, not IP.** MVP identity from #2 is `Ed25519 keypair → PeerId (12D3Koo…/bafz…)`. Transport must authenticate that identity at handshake (not at app layer via JWT).
- **Discovery without DNS.** When a new node joins, it knows ≤1 bootstrap `Multiaddr` + embedded `/p2p/<PeerId>`. From there it must discover other peers and their listen addrs.
- **Rust-preferred, Tokio-based.** One `#[tokio::main]` runtime should host agent HTTP + swarm + storage. MSRV should remain reasonable (1.75–1.83).
- **Ledger traffic pattern.** Transfers are request/response (submit tx, fetch balances/history). Gossip is useful later for reputation/ledger propagation. Bidirectional streams are needed eventually but not day-one.
- **Frugal on bandwidth / latency.** Ledger messages are small; overhead of headers, TLS handshakes and varint framing matters less than NAT reachability.

These constraints immediately separate the candidates: HTTP/gRPC/WebSocket are *reachability-oblivious* — they work iff the callee is diallable. libp2p is the only candidate that *owns* reachability.

---

## 2. Protocol Comparison at a Glance

| Dimension | **HTTP/REST+JSON** (`axum`, `hyper`, `reqwest`) | **gRPC** (`tonic` 0.12, `prost`) | **WebSocket** (`axum::extract::ws`, `tokio-tungstenite`) | **libp2p** (`libp2p 0.54.1+`) |
|---|---|---|---|---|
| **Framing / wire** | HTTP/1.1 or H2, JSON bodies, status codes, headers | HTTP/2 frames, protobuf (binary) `application/grpc`, trailers/status, unary + streaming RPC | TCP + `Upgrade: websocket` → persistent frames (text/binary), ad-hoc JSON-RPC inside | Transport-agnostic: `Multiaddr` → `TCP`/`QUIC`/`WebSocket`/`WebTransport`/`WebRTC` negotiated via `multistream-select`; varint-length-prefixed messages, `yamux`/`mplex` multiplexing |
| **Multiplexing** | HTTP/2 multiplexed streams if using H2; H1 is 1-req-at-a-time per conn | Native H2 multiplexing: many concurrent streams per conn, flow control, cancellation | One ordered reliable channel; app must multiplex if it wants parallel RPCs | `yamux` (default) or `mplex` over a single encrypted connection; many concurrent protocols/streams |
| **Encryption / auth** | TLS via `rustls`/`hyper-rustls`, or `plaintext` for localhost; identity via external JWT/mTLS you build | TLS via `rustls` (tonic `tls`), or insecure; identity via mTLS cert CN — not PeerId | TLS via `wss://` wrapper if any; identity via app-token or sub-protocol header | **Built-in:** `libp2p-noise` (`Noise_XX_25519_ChaChaPoly_SHA256`, XX pattern) or `libp2p-tls` (self-signed X.509 extension `OID 1.3.6.1.4.1.53594.1.1`); handshake binds static DH key to long-term Ed25519 identity via `identity_sig` and aborts before app bytes if PeerId mismatch |
| **Identity** | IP:port or external registry; no PeerId concept | TLS cert CN/SAN or auth interceptor; no PeerId | Client-supplied `Host`/`Sec-WebSocket-Protocol`; no PeerId | **Native:** `libp2p_identity::Keypair::generate_ed25519()` → `PeerId = multihash(protobuf(public_key))`; `identity` multihash for Ed25519 (≤42 B), `sha2-256` for RSA; text as `12D3Koo…` (base58btc) or `bafz…` (CIDv1 `libp2p-key` base32); `PeerId::is_public_key()` verifies without lookup |
| **Discovery** | None — you must know `http://host:port`. Service mesh / Consul / DNS are external systems. | None — see HTTP. `grpcurl` reflection lists *methods*, not peers. | None. | **Bundled:** `identify` (pushes `public_key`/`listen_addrs`/`observed_addr`/`protocols` on every new connection), `kad` (Kademlia DHT for `put_record`/`get_record`/`add_address`/`bootstrap`, replicating DHT for registry/ledger hints), `mdns` (multicast `_p2p._udp.local PTR` → `dnsaddr=.../p2p/<PeerId>` for LAN, zero-config), `rendezvous` (namespaced registry for bootstrapping) |
| **NAT traversal** | **None.** If callee is behind NAT, caller cannot dial `IP:port`. Needs reverse proxy / public relay you pay for. | **None** — inherits HTTP/TCP reachability limits. Proxy/mesh (Envoy, Istio) requires infra on both sides of NAT. | **None** — client behind NAT can dial out, but cannot be dialled. No hole-punching. | **Stacked:** `autonat` (are-I-public? ask peers to dial back your `ObservedAddr`), `relay v2` (`hop`+`stop`, reservation vouchers, `Limit {duration,data}`, fallback when hole-punch fails; `/p2p/<relay>/p2p-circuit/p2p/<target>`), `dcutr` (hole-punch upgrade over relay: `Connect`/`Sync` + simultaneous dial with `RTT/2` delay, TCP simultaneous-open + QUIC packet spray), `upnp` (gateway port-map), `identify`'s `observed_addr` as decentralized STUN |
| **Streaming / push** | Poll / long-poll / SSE (`text/event-stream`); no server-initiated RPC | Native streaming: unary, server-streaming, client-streaming, bidi-streaming over one H2 conn | Full duplex, server can push at any time (one ordered channel) | App chooses: `gossipsub` (pubsub mesh for ledger propagation), `request-response` (typed req/resp with cbor/json), libp2p streams (`/ai-bank/transfer/1.0.0`) — any number of concurrent bidi streams over yamux |
| **Schema / contracts** | Ad-hoc JSON; `schemars`→ `JsonSchema` → OpenAPI via `utoipa` | `proto3` `.proto` file → `tonic-build` + `prost` code-gen; strong typing, `grpcurl`/`buf` tooling | Ad-hoc (you invent framing: JSON-RPC, `rmp-serde`, `serde_json` per frame) | Multistream-negotiated protocol ids (`/ai-bank/transfer/1.0.0`, `/ai-bank/ledger/1.0.0`, `/ipfs/kad/1.0.0`); app chooses `serde`/`prost`/`cbor` codec per protocol; not lock-step with proto toolchain |
| **Browser reachability** | `reqwest` needs `cors` for browsers; plain HTTP behind NAT still unreachable | gRPC-Web bridge required for browsers (`tonic-web` `GrpcWebLayer`, `accept_http1:true` for h2c) | Browser native `WebSocket` (4–5 RTT including TLS+upgrade+Noise) | `websocket` + `webtransport-websys` + `webrtc-websys` transports exist (`/webrtc`, `/webrtc-direct`); avoids CA transplant via cert-hash-in-`Multiaddr` + Noise-on-first-stream |
| **Operational cost** | Free if you run the HTTP server; discovering/NAT-traversing needs extra paid infra | Same as HTTP, plus proto toolchain | Same as HTTP | **Free-data-plane option:** relay is community-run (limited `duration`/`data` caps so any public node can be a relay at minimal cost); no mandatory STUN/TURN (uses peers themselves); larger relay mesh ≈ more scale-out without paying AWS/GCP bills |
| **Complexity** | Low: `Router::new().route(...)` + `tokio::net::TcpListener::bind(...)` | Medium: `.proto` compilation step, `Routes::builder` → `into_axum_router` merging, `Content-Type: application/grpc` routing | Low to start; high when you reinvent reconnect/heartbeat/ordering | **Medium-high up front** (Swarm builder, `NetworkBehaviour`, `Multiaddr`, event loop), but absorbs NAT/discovery/identity complexity you'd otherwise rebuild |

Sources: `libp2p/specs/peer-ids/peer-ids.md` (keys, PeerId derivation, base58/CID encodings); `libp2p/specs/noise/README.md` (XX pattern, `Noise_XX_25519_ChaChaPoly_SHA256`, static-key signature payload `identity_key`/`identity_sig`); `libp2p/specs/relay/circuit-v2.md` (hop/stop split, `RESERVE`/`CONNECT`/`STATUS`, `Reservation{expire,addrs,voucher}`, `Limit{duration,data}`, vouchers `libp2p-relay-rsvp`); `libp2p/specs/relay/DCUtR.md` (hole-punch `Connect`/`Sync`, `Sync`+`RTT/2`, TCP simultaneous-open vs QUIC spray); `libp2p` docs.rs for `kad`/`identify`/`autonat`/`relay`/`dcutr`/`mdns`/`gossipsub`; `tokio-rs/axum 0.8.9` docs.rs (routing, tower, MSRV 1.80, `forbid(unsafe_code)`); `hyperium/tonic` docs.rs (HTTP/2, `transport`/`server`/`channel`/`router`, `Routes::into_axum_router`); `rust-libp2p v0.55.0` features table and `SwarmBuilder` example used below.

---

## 3. Deeper Trade-offs per Protocol

### 3.1 HTTP/REST+JSON (`axum` / `hyper`)

HTTP/REST is the universal reachability for **agents**. Ticket #3 already locks it in on `127.0.0.1` for agent↔node, and that choice is correct to keep. For node↔node, the same simplicity becomes the limitation:

- **No reachability story.** A `reqwest::get("http://192.168.1.42:3000/balances/...")` fails if `192.168.1.42` is behind a NAT that did not port-forward. You would need each user to expose a public IP or rent a reverse tunnel. "No cloud bills" forbids the natural fix.
- **No identity binding.** You would have to add JWTs or mTLS certs and a registry that maps `PeerId → http://host:port`. That registry becomes a server again.
- **Tooling advantage preserved elsewhere.** `curl`, `httpie`, browser fetch, `utoipa` OpenAPI all stay available on the localhost API. For inter-node, OpenAPI is overhead versus a small typed `request-response` protocol.

When HTTP *does* make sense for inter-node: a debug/admin sidecar bound to `127.0.0.1` on each node, or a metrics/health endpoint scraped locally. Not as the ledger transport.

### 3.2 gRPC (`tonic` + `prost`)

`tonic` is an excellent **backend-interior** framework: code-gen contracts, deadlines, metadata, trailers, streaming, `tower` middleware, `rustls` TLS, and `Routes::into_axum_router` so it can share a `tokio` listener with `axum` via `Content-Type: application/grpc` dispatch. For pure request/response between two public-IP services, gRPC is strictly better than REST on overhead.

For AI Bank P2P, it inherits HTTP's reachability problem and adds one more:

- **Still needs diallable peers.** "gRPC is built on TCP and inherits all of its reachability limitations. The standard workaround is a gRPC proxy (like Envoy) or a service mesh (like Istio). Both require infrastructure you control on both sides of the NAT." [Pilot Protocol comparison — TCP→gRPC note]
- **No discovery, no identity.** gRPC reflection enumerates *methods*; finding *which IPs host `ai-bank.Transfer`* is left to Consul/etcd/k8s DNS.
- **Build toolchain tax.** `.proto` edits → `tonic-build`/`prost` → generated code → drift if language bindings differ.

That said, gRPC **is compatible** with libp2p if you later want typed RPC contracts over a NAT-traversed stream. The community crate `libp2p-grpc-rs` exposes a `NetworkBehaviour` + `DirectGrpcUpgrade` so a `tonic::Server`/`Channel` pair runs over a libp2p yamux stream (protocol id `/grpc/1.0.0` style), keeping `prost` contracts while outsourcing reachability to libp2p. Treat this as a later composition, not a replacement for libp2p.

### 3.3 WebSocket (`axum::extract::ws`, `tokio-tungstenite`, `libp2p-websocket`)

WebSockets give you one ordered, reliable full-duplex channel after a 1–2 RTT upgrade (5 RTT with `wss` + Noise/TLS). For dashboards and server push they are right.

For inter-node sync they again punts on the hard parts:

- NAT traversal still absent. A node behind NAT can open `ws://relay.example.com` but another NATed node cannot dial `ws://natPeer:4000`. `libp2p-websocket` as a *transport* (i.e., `libp2p::websocket`) mitigates this by running the WebSocket handshake *inside* an already-established libp2p connection, not as a standalone discovery mechanism.
- Ordering head-of-line blocking on a single channel means you would need to multiplex app protocols yourself or run one WS connection per protocol.
- Extra latency vs TCP: "WebSocket Secure connection … 4 round trips listed above plus another round trip for the TLS handshake, increasing the handshake latency to 5 RTTs." Plain `ws` is cheaper, but still more than TCP or QUIC under libp2p.

Where WebSockets *do* fit: the optional browser tier. `libp2p-websocket-websys` + `libp2p-webtransport-websys` + `libp2p-webrtc-websys` are first-class transports for "browser as node" — but they still depend on STUN + relay + signalling run over an existing libp2p relay connection, not on WebSockets being the core protocol.

### 3.4 libp2p (Why It's Different — Transport-Agnostic, Identity-Native)

libp2p is not "another protocol over TCP" — it is a **stack**: multiaddr → transport → security → multiplexing → discovery → relay → app-protocol. The comparison above makes libp2p look more complex only because it actually *attempts* the three hard problems the others delegate to ops.

Key discriminator: *libp2p's NAT traversal draws from ICE but removes the dependency on centralized STUN/TURN servers, using distributed coordination instead* — `AutoNAT`≈STUN, `identify`'s `observed_addr`≈decentralized STUN, `Circuit Relay v2`≈TURN but lightweight (`duration`/`data` capped and reservation-vouchered), `DCUtR`≈signalling-free hole-punch.

---

## 4. libp2p Specifics for AI Bank (0.54+, with 0.55/0.56 notes)

Everything below is available as opt-in feature flags on `libp2p 0.54.1` (`crates.io — libp2p 0.54.1`, MSRV `1.75.0`) and `0.55.0`/`0.56.0` (`MSRV 1.83.0`). No default feature enables anything; you pick the stack you want via `features = ["tokio","tcp","quic","noise","yamux","quic","identify","kad","autonat","relay","dcutr","dns","mdns","gossipsub", …]` (see `docs.rs/crate/libp2p/0.55.0/features` table — 38 flags, `full` enables all).

### 4.1 Identity — `PeerId` from Keypair

- Keys serialized as a deterministic `PublicKey { Type, Data }` protobuf (fields varint-minimal, tag-ordered, no unknown fields). Supported `Type`: `RSA`/`Ed25519`/`Secp256k1`/`ECDSA`; implementations MUST support `Ed25519`. RSA uses DER PKIX, Ed25519 uses RFC 8032 64-byte `Sign(priv, msg)` (no extra encoding), Secp256k1 uses BIP-0062 DER. Private keys never leave the host — stored on disk as `PrivateKey` protobuf (Ed25519 ` [priv 32 | pub 32]` or legacy `96` bytes duplicate check). [Source: `peer-ids/peer-ids.md` — Keys/Key Types]
- `PeerId` = `multihash( protobuf(public_key) )`. If serialized ≤42 bytes (Ed25519 case → 36 bytes), multihash codec `identity` is used (identity function — just wrapped bytes, no digest); otherwise `sha2-256`. String forms: legacy `base58btc` (`Qm…` for sha256, `12D3Koo…` for identity) and new CIDv1 `libp2p-key` (`0x72`) in base32 (`bafz…`). MUST parse both, SHOULD display `base58btc` until CID is widespread. Verify via `PeerId::is_public_key(&key)` / `PeerId::from_public_key`. [Source: `peer-ids.md` — Peer Ids, Encoding/Decoding; `libp2p/rust-libp2p/core/src/peer_id.rs`]
- **Depends on #2:** If MVP chooses Ed25519-only (recommended), `PeerId` is `12D3KooW…` style and no registry mapping is security-critical — the display alias stays local.

### 4.2 Transports & Upgrades

- **Base transports:** `libp2p-tcp` (`Config::default().port_reuse(true).nodelay(true)`), `libp2p-quic` (`0.12.x`), `libp2p-dns` (wraps other transports, must be composed before relay), `libp2p-uds` (unix sockets for local debugging), `libp2p-websocket`/`webtransport-websys`/`webrtc-websys` for browsers.
- **`SwarmBuilder` guidance (0.55+):** new type-safe builder removes DNS-before-relay ordering pitfalls:
  ```rust
  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
      .with_tokio()
      .with_tcp(tcp::Config::default().port_reuse(true).nodelay(true),
                noise::Config::new, yamux::Config::default)?
      .with_quic()
      .with_dns()?
      .with_relay_client(noise::Config::new, yamux::Config::default)?
      .with_behaviour(|keypair, relay_client| Behaviour {
          relay_client, ping: ping::Behaviour::default(),
          dcutr: dcutr::Behaviour::new(keypair.public().to_peer_id()),
      })?
      .build();
  ```
  [Source: `rust-libp2p` releases — SwarmBuilder example]
- **Security:** `libp2p-noise` (XX) or `libp2p-tls` (X.509 extension `1.3.6.1.4.1.53594.1.1`). Noise uses a separate ephemeral X25519 `static` DH key per handshake, authenticated by `NoiseHandshakePayload { identity_key, identity_sig, extensions }` where `identity_sig = Sign(identityPriv, "noise-libp2p-static-key:" + x25519PubLE)` using Ed25519/RSA/ECDSA per peer-ids spec; `Supported cipher suite: Noise_XX_25519_ChaChaPoly_SHA256`. TLS variant signs `libp2p-tls-handshake:<SPKI>` and stuffs the public key in the extension, then derives PeerId from it. Both abort before app data if signature invalid or expected PeerId mismatched (PR #4864). [Source: `specs/noise/README.md` — Handshake, Static Key Authentication, Protocol Name; `specs/tls/tls.md`]
- **Multiplexing:** `libp2p-yamux` (preferred, reliable, flow-controlled streams over one encrypted connection) or `mplex` (deprecated pathway; new work favors yamux).

### 4.3 NAT Traversal — The Decisive Stack

| Component | Protocol id / crate | What it does | Why it matters for "no cloud bills" |
|---|---|---|---|
| `identify` | `/ipfs/id/1.0.0`, `libp2p-identify` | On every connection push, exchange `{ public_key, listen_addrs, observed_addr, protocols{...} }`. Receiver's `PeerStore` stores `{PeerId → addrs}`. Also pipes `observed_addr` for peers to learn their external `IP:port`. | Decentralized STUN: uses existing peers, not a `stun.l.google.com` you fund. Needed by every other NAT component. |
| `autonat` (`v1`/`v2`) | `libp2p-autonat`, clients ` /libp2p/autonat/1.0.0/dial` | Asks connected peers to dial back the addrs you advertise. Reachable → `Public`, not → `Private`. `autonat` → reject inbound dial request if not connected (DoS mitigation). `autonat-v2` adds `dial_back_to_non_libp2p` robustness. | Self-classification "am I behind NAT?" without your own stun infra. Decide whether to reserve a relay slot. |
| `relay v2` (Circuit Relay) | `hop` `/libp2p/circuit/relay/0.2.0/hop` + `stop` `/libp2p/circuit/relay/0.2.0/stop`, `libp2p-relay` | Client sends `HopMessage { RESERVE }` → relay replies `STATUS OK { reservation{expire,addrs,voucher}, limit{duration,data} }`. Later `CONNECT to A` via relay's `hop` → relay opens `stop` to `A` → streams are bridged (`B --hop--> R --stop--> A`), then upgraded with Noise+multiplexer like a normal transport. Reservation vouchers are signed envelopes (`libp2p-relay-rsvp`, multicodec `0x0302`, payload `{relay, peer, expiration}`). | Distributed TURN-but-cheap: *limited relays* with per-connection `duration`/`data` caps and expiry so any public node can be a relay at negligible cost. Scales horizontally by "army of relays" rather than one hosted relay you pay for. Addresses learned: `…/p2p/<relay>/p2p-circuit/p2p/<target>` via `addrs` in voucher. |
| `dcutr` | `/libp2p/dcutr`, `libp2p-dcutr` | Hole-punch upgrade without a signalling server. After relay connection exists, `B` opens `/libp2p/dcutr`, sends `HolePunch { CONNECT { ObsAddrs: [*multiaddr] } }` and starts RTT timer; `A` replies `CONNECT`; `B` sends `SYNC` and starts `RTT/2` timer; simultaneous dial: `A` dials immediately on `Sync`, `B` dials after timer — TCP simultaneous-open (dual dial) or QUIC packet spray (10–200 ms random interval, random bytes). Single success → peers migrate new streams to direct conn, close relay after grace. Retries ≤3. | Without this, all NAT traffic rides the relay forever (expensive). With it, peers reuse the relay only as signalling, then go direct — the advertised latency payoff with no extra server. |
| `upnp` | `libp2p-upnp`, feature `upnp` | Tries `InternetGatewayDevice` port-mapping via UPnP-NAT-PMP. | Opportunistic: when home router cooperates, NAT traversal is free. Do not rely on it. |
| `quic` | `libp2p-quic 0.12` (or `0.13` in 0.56 stack) | UDP-based QUIC helps NATs that are nicer to UDP hole-punching than TCP simultaneous-open. | Some NATs that drop TCP simultaneous-open still permit QUIC spray to succeed. Enable TCP+QUIC together. |

[Sources: `specs/relay/circuit-v2.md` — hop/stop protocols, reservation/limit/voucher protobuf; `specs/relay/DCUtR.md` — `Connect`/`Sync`/`RTT/2` simultaneous dial, QUIC spray semantics; `libp2p.io/docs/dcutr/`, `rust-libp2p#5357` DCUtR vs relay vs rendezvous vs autonat discussion; `crates.io libp2p 0.55.0/0.54.1` feature tables (autonat, relay, dcutr, upnp, identify, quic, tcp, dns, yamux, noise, tls, tokio, kad, mdns, gossipsub).]

**Limitation to document:** Symmetric / endpoint-dependent-mapping NATs (many CGNATs) *cannot* be hole-punched — even ICE/DCUtR need a relay fallback, and `BEHAVE RFC4787 REQ-1` asks for endpoint-independent mapping to avoid this. For MVP, accept relay-only as the fallback for ~5–15% of nodes and show UX `relay-only (limited)` indicator.

### 4.4 Discovery & Address Book

- `identify` → `PeerStore` is the address book. libp2p intentionally does **not** auto-wire Identify into Kademlia: *Rust-libp2p tries to stay as generic as possible — the Identify protocol must be manually hooked up to Kademlia through calls to `Behaviour::add_address`. Without Identify or an alternative, a Kademlia node will not discover nodes beyond boot nodes.* Wire it: on `identify::Event::Received { peer_id, info: { listen_addrs, observed_addrs } }`, call `kad.add_address(&peer_id, addr)` for each. [Source: `docs.rs libp2p_kad` — Peer Discovery with Identify discrepancy note]
- `kad` (`libp2p-kad` / `libp2p-kad 0.47/0.48`) implements the libp2p-specific Kademlia DHT (bucket size `k=20`, iterative lookup). Use `Mode::Server` for any node willing to store routing table entries (default some examples start as `Mode::Client`). Operations: `bootstrap()`, `get_closest_peers(Key)`, `put_record(Record{key,value,publisher,expires})`, `get_record(Key, Quorum)`, `start_providing(Key)`/`get_providers`. For AI Bank registry: either store `PeerId → Multiaddr` as a provider record, or `RecordKey("/ai-bank/peers/<peer-id>") → Multiaddr` as a value record. Replication factor defaults to 20; consider `Caching` (write-back) after successful lookups.
- `mdns` (`0.47/0.48`, features `tokio`/`async-std`) uses multicast DNS-SD service `_p2p._udp.local` — `PTR → TXT dnsaddr=…/p2p/<PeerId>` with additional `SRV/A/AAAA` records. Zero-config LAN discovery: two nodes on the same Wi-Fi find each other with no bootstrap. Ignores loopback / link-local from outside LAN; random 32+ char lower-case `peer-name` (not PeerId) for DNS label (<63 chars). [Source: `specs/discovery/mdns.md`; `libp2p.io/docs/mdns/`]
- `rendezvous` (namespaced registry, feature `rendezvous`) is an alternative bootstrap-friendlier registry than raw KAD for small networks: peers `register(namespace, ttl)` at a known rendezvous point and others `discover(namespace)`. Add later if KAD feels too heavy for the first 10-node pre-genesis network.

---

## 5. Rust Ecosystem Support (0.54+, tonic, axum)

All crates below are async, `tokio`-native, and can coexist on a single `#[tokio::main]` with one Tokio runtime — the node can run swarm + HTTP side-by-side without a second executor.

| Approach | Crate(s) | Current version 2026-09-02 (crates.io) | MSRV / key deps | Maturity / ergonomics |
|---|---|---|---|---|
| **libp2p** (node↔node) | `libp2p`, `libp2p-identity`, `libp2p-noise`/`libp2p-tls`, `libp2p-yamux`, `libp2p-tcp`/`libp2p-quic`/`libp2p-dns`/`libp2p-uds`, `libp2p-kad`, `libp2p-identify`, `libp2p-autonat`, `libp2p-relay`, `libp2p-dcutr`, `libp2p-mdns`, `libp2p-gossipsub`/`libp2p-request-response`, `multiaddr`, `multihash`, `bs58` | `libp2p 0.54.1` (2024-08-19) / `0.55.0` (2025-01-15) / `0.56.0` (2025-06-27); sub-crates as listed across `crates.io — libp2p 0.55.0` (e.g. `libp2p-kad 0.47/0.48`, `libp2p-gossipsub 0.48/0.49`, `libp2p-noise 0.46`, `libp2p-yamux 0.47`) | `libp2p 0.54.1` MSRV `1.75.0`, `0.55.0`/`0.56.0` MSRV `1.83.0`; `libp2p-identity 0.2` uses `ed25519-dalek 3.x` under `ed25519` feature; `libp2p-swarm 0.47`; `SwarmBuilder` type-state was added in the 0.54→0.55 cycle | Most downloaded P2P stack in Rust (`4M+` downloads for `0.54.1` alone); used by `iroh`/`substrate`/`lighthouse`; `examples/` include `autonat`, `autonatv2`, `relay`, `dcutr`, `distributed-key-value-store` (kad+mdns), `chat` (gossipsub+mdns). `libp2p-core 0.43` + `libp2p-swarm 0.47` + `multihash 0.19`. |
| **HTTP/REST** (agent↔node localhost) | `axum 0.8`, `tokio 1`, `serde`/`serde_json`, `schemars`, `utoipa`, `tower-http` | `axum 0.8.9` (2026-04-14, `tokio-rs/axum`), `hyper 1.4`, `hyper-util 0.1`, `tower 0.5`, `tower-http 0.6` | MSRV `1.80`; `#![forbid(unsafe_code)]`; macros-free routing (`/{id}` syntax). Axum shares `tower::Service` with `tonic` so middleware is reusable. | Dominant Rust HTTP framework (450M total downloads, 53M for 0.8.9); Tower middleware gives timeouts/tracing/compression/authz for free. |
| **gRPC** (opt-in later) | `tonic 0.12`, `tonic-build`, `prost 0.13`, `tonic-web`, `tonic-health`, `tonic-reflection` | `tonic 0.12.3` current stable; `tonic 0.13` in pre-release track; `tonic-web 0.12.3` for gRPC-Web (`GrpcWebLayer`, requires `accept_http1:true` over cleartext) | Hyper 1.4 + Tower 0.5 + Tokio 1; prost prost-types 0.13; feature `transport` enables `server`+`channel` (H2 + TLS via `rustls`), feature `router` enables axum `Routes::into_axum_router`. MSRV ~= 1.75; `Routes::builder().add_service(...).routes().into_axum_router()` is documented pattern for multiplexing gRPC + axum on one listener. | `hyperium/tonic` is the canonical Rust gRPC (interop tests, reflection, health). Works seamlessly with `axum::serve(listener, routes)` — tonic supplies `Routes`, axum supplies REST. Pitfall that `tonic::transport::Server::builder().into_router()` is now deprecated in favour of `Routes` — migrating examples hit body-type mismatches when `GrpcWebLayer` is applied outside `Routes`. |
| **WebSocket** (browser tier / push) | `axum` (`ws` feature), `tokio-tungstenite 0.26`, `libp2p-websocket`/`websocket-websys` | `tokio-tungstenite 0.26` (pairs with `axum 0.8`), `libp2p-websocket 0.44/0.45`, `libp2p-webtransport-websys 0.5`, `libp2p-webrtc-websys 0.4` | `axum` ws upgrades: `WebSocketUpgrade` extractor → `socket.recv()` loop | Axum's `ws` extractor streams `tungstenite` frames; `jsonrpsee 0.24` can layer JSON-RPC over WS if needed (`RpcModule`). Libp2p ws/webtransport are separate *transports*, not app protocols. |
| **libp2p→gRPC bridge** (optional composition) | `libp2p-grpc-rs` (`Behaviour` + `DirectGrpcUpgrade`) | Pre-1.0 community crate; depends on `libp2p-swarm` + `tonic::transport` | Re-exports `tonic` service definition, lets `tonic::Server`/`Channel` run over yamux | Useful if you want prost contracts *and* NAT traversal: write `.proto` + `tonic` as usual, then `Behaviour` handles discovery/NAT. Do not block MVP on it. |

**Interop note:** `tonic` and `axum` can share one listener via content-type dispatch:

```rust
use tonic::service::Routes;
let grpc_router = Routes::builder()
    .add_service(greeter_server::GreeterServer::new(svc))
    .routes().into_axum_router();
let app = axum::Router::new()
    .route("/health", get(health))
    .merge(grpc_router);
axum::serve(listener, app).await?;
```

...but for node↔node you would still need a dialable `listener.local_addr()` on the public Internet — which NAT breaks. Hence the libp2p path remains distinct from this axum+tonic merge pattern.

---

## 6. Interaction with Node Identity (#2)

> **Dependency on #2:** Ticket #4 assumes the choice from #2 is *Ed25519 keypair → libp2p PeerId* with a local petname table. This section records the consequences and fallback if #2 had chosen differently, and points to its file.

**What #2 decided:** `libp2p_identity::Keypair::generate_ed25519()` on first run → `Keypair::to_protobuf_encoding()` → `~/.ai-bank/identity.key` (`0o600`) → reload via `from_protobuf_encoding`. Canonical string is `peer_id.to_base58()` (`12D3Koo…`) or CID `bafz…`; display layer shows `alias (PeerId abbr)` from local `~/.ai-bank/peers.json` (Spritely petname pattern — petname/edge/self-proposed). Transport handshake (Noise XX or TLS Public Key Extension OID `1.3.6.1.4.1.53594.1.1`) binds the ephemeral session key to that long-term identity via `Protocol::sign/verify`; abort precedes app bytes if PeerId mismatch. App messages sign `b"/ai-bank/1/transfer:" || cbor(tx)` with same key and verify via `PublicKey::verify`. [Context pointer: branch `research/node-identity`, file `docs/research/node-identity.md`; commit `047a67e`.]

**How that choice interacts with the communication protocol:**

| Coupling | libp2p | HTTP/REST / gRPC / WebSocket |
|---|---|---|
| **Producing PeerId** | **Zero extra code.** `Keypair → PeerId` is `PeerId::from_public_key(&keypair.public())` which is `multihash(protobuf(pubkey))` per `peer-ids.md`. PeerId string is deterministic and self-verifiable. | You would serialize the Ed25519 public key as a header (`x-ai-bank-peer-id: 12D3Koo…`) or as a field in JSON/proto and rebuild `PeerId::from_bytes` on arrival. No handshake-level check — you reinvent `is_public_key`. |
| **Verifying sender** | **Automatic in handshake.** Noise payload `identity_key` + `identity_sig` links X25519 static key to long-term identity; `InboundSecurityUpgrade` verifies signature and (when dialing a known peer) that the revealed `PeerId` matches the expected `/p2p/<PeerId>` in the Multiaddr. No app code. | Must add middleware: extract `x-peer-id` + `x-signature` per request, call `PublicKey::verify` + `PeerId::is_public_key`, and maintain a manual allow-list. Every endpoint repeats the check. |
| **Key types** | libp2p *must* support `Ed25519`; MAY support `RSA`/`Secp256k1`/`ECDSA`. Choosing Ed25519-only keeps `PeerId` as `identity` hash (≤42 B) and signatures as pure Ed25519 RFC8032. Switching later to RSA would change PeerId encoding to `sha2-256` and base58 `Qm…` style — a visible migration. | Same migration pain, but you also need to change the header/codec. |
| **Rotation / new keys** | New `Keypair` → new `PeerId` — no built-in indirection. Roadmap: optionally publish a signed `old → new` Statement (`SignedEnvelope` domain `ai-bank/rotation/1`, `payload { old_peer:"12D3…", new_peer:"12D3…", seq, sig }`) stored in `kad` so peers can follow the move. Until that exists, rotation means re-introducing. Same for any protocol. | Same. HTTP/gRPC would need a similar registry record — but without `kad` you need a server to store it. |
| **Petnames vs global names** | `libp2p-identify` + `kad` + `Signed Peer Records` (`CertifiedAddrBook::ConsumePeerRecord`) already provide self-certified `{PeerId, addrs, seq, signature}` hints without a CA. Display `alias` stays local; shared `aliases[]` are hints verified with `PublicKey::verify`. | You'd re-invent a CA or a global `peers.json` served from a static site and ask users to trust its `https` origin. |
| **TLS parity** | `libp2p-tls` stuffs Ed25519 pubkey into the SPKI extension so standard H2-capable transports can still prove PeerId — useful if you later expose an `https://…/libp2p` WebTransport. | Plain `rustls` mTLS certs bind `CN = PeerId` but need issuance/renewal. No built-in PeerId derivation. |

**If #2 had chosen "human names only" (rejected):** Node↔node would have to resolve `alice.ai-bank → 1.2.3.4:4001` via a hosted registry (violates no-cloud). libp2p would lose its native `PeerId` verification — you would be forced into HTTP/gRPC with a bespoke nameserver. The fact that #2 chose crypto-first is precisely what makes libp2p the low-friction pick.

**If #2 had chosen hybrid without fixing crypto type:** Same table, but the libp2p path would need explicit `KeyType` negotiation up front (Ed25519 vs Secp256k1). Sticking to Ed25519-only for MVP simplifies both identity and transport.

---

## 7. What Works Without Cloud / Bills and Handles Users Behind NAT

Scoring the four candidates against the AI Bank constraints (✅ = native / negligible cost, ⚠️ = possible but needs server you pay for, ❌ = not supported):

| Constraint | HTTP/REST | gRPC | WebSocket | **libp2p** |
|---|---|---|---|---|
| Runs on user's machine, no daemon you pay for | ✅ (you ship `ai-bank serve`) | ✅ | ✅ | **✅** |
| Nodes behind NAT can receive dials | ❌ (needs public IP or forwarded port) | ❌ (same) | ❌ (client can dial out, not in) | **✅ with relay v2 + dcutr; relay load is capped** |
| Symmetric-NAT / heavy CGNAT fallback | ❌ | ❌ | ❌ | ⚠️ (stays on relay — limited `duration`/`data`; works but slower) |
| Works on LAN with no Internet | ⚠️ (if you know the LAN IP, e.g. `192.168.1.10:3000`) | ⚠️ (same) | ⚠️ (same) | **✅ mDNS zero-config** |
| Works offline/disconnected | ❌ | ❌ | ❌ | Partial (local `ledger` ops work; replication resumes on reconnect) |
| Peer identity authenticated at handshake | ❌ (need custom) | ❌ (mTLS CN trick) | ❌ (need custom) | **✅ Noise/TLS binds PeerId** |
| Peer discovery without central DB | ❌ (need Consul/ETCD) | ❌ (need Consul/ETCD) | ❌ | **✅ `identify` + `kad` + `mdns` + `rendezvous`** |
| Browser clients w/o CA pain | Needs CORS, mixed-content, CA | Needs gRPC-Web proxy + CA | Works but needs `wss` + CA + 5 RTT | **Transports encode cert hash in multiaddr + Noise on first stream** (3 RTT for WebTransport) |

**Why libp2p is the only realistic choice for node↔node:** HTTP/gRPC/WebSocket optimize for *request purity* — they assume reachability is someone else's problem. AI Bank's problem *is* reachability. libp2p pays for it once: relay reservations (lease-then-bridge), timed simultaneous dial, opportunistic UPnP — all use peers themselves, not a cloud STUN/TURN fleet. The fallback ("can some node with a public IP be the relay?") scales by inviting any public node to opt in (`libp2p-relay` server is a ~30-line Behaviour), not by paying for a relay fleet. This is the "army of relays for extreme horizontal scaling without excessive bandwidth costs and dedicated hosts" rationale in `circuit-v2.md`.

**No-bills nuance:** A "real" deployment will still benefit from *one* well-known bootstrap rendezvous node so new peers can bootstrap quickly. That node *is* a small cost (a single cheap VPS or a user's always-on desktop), but it is not a required bill for *every* user and the network degrades gracefully without it (mDNS on LAN, peer exchange via already-connected peers). Document this distinction for ADR — bootstrap ≠ CA.

---

## 8. Recommendation for AI Bank MVP

### Decision: Ship node↔node as **libp2p swarm**; keep **axum HTTP on `127.0.0.1`** for agent↔node. Do not ship gRPC/WebSocket as primary node↔node for MVP.

#### 8.1 Concrete structure (Rust, one Tokio runtime)

```toml
# Cargo.toml (excerpt, MSRVs pinned for 0.54 path; bump to 1.83 when adopting 0.56)
[dependencies]
libp2p            = { version = "0.54.1", features = ["tokio","tcp","quic","dns","noise","yamux","identify","kad","autonat","relay","dcutr","mdns","gossipsub","macros","ed25519"] }
libp2p-identity   = { version = "0.2" }  # re-exported by libp2p; explicit if signing outside swarm
multiaddr         = "0.18"
multihash         = "0.19"
bs58              = "0.5"
tokio             = { version = "1", features = ["full"] }
axum              = { version = "0.8", features = ["json","http1","http2"] }  # localhost control plane (#3)
serde             = { version = "1", features = ["derive"] }
serde_json        = "1"
schemars          = { version = "1", features = ["derive"] }
clap              = { version = "4", features = ["derive"] }
```

```rust
// src/net/behaviour.rs
use libp2p::{autonat, dcutr, identify, kad, gossipsub, ping, relay, swarm::NetworkBehaviour};

#[derive(NetworkBehaviour)]
pub struct BankBehaviour {
    pub relay_client: relay::client::Behaviour,
    pub identify: identify::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub autonat: autonat::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub mdns: mdns::tokio::Behaviour,        // LAN
    pub gossipsub: gossipsub::Behaviour,     // ledger propagation
    pub ping: ping::Behaviour,
    // later: pub request_response: request_response::cbor::Behaviour<TransferReq, TransferRes>
}

// src/net/swarm.rs — builder snippet (see §4.2)
// .with_behaviour(|keypair, relay_client| Behaviour { relay_client, ping: ..., dcutr: ... })?
```

`Module layout`

```
src/
  identity/          # wraps libp2p_identity — generate Ed25519, write/read ~/.ai-bank/identity.key, PeerId helpers (assumes #2)
    keypair.rs
    peer_id.rs
  net/
    swarm.rs         # SwarmBuilder::with_tokio + transports + Behaviour wiring, listen_on("/ip4/0.0.0.0/tcp/0", "/ip4/0.0.0.0/udp/0/quic-v1")
    behaviour.rs     # BankBehaviour above
    discovery.rs     # identify→kad add_address hook, bootstrap(), mdns→kad promotion
    nat.rs           # AutoNAT status handling → relay RESERVE decision + UPnP attempt
    relay.rs         # RESERVE/refresh loop, voucher logging, CONNECT bridging feedback
    dcutr.rs         # observe relay connection → DCUtR Connect/Sync events
    gossip.rs        # gossipsub topics "/ai-bank/transfer/1.0.0", "/ai-bank/registry/1.0.0", message signing
    rpc.rs           # request-response codec for submit/fetch (cbor or serde_json)
  service/           # pure ledger logic — no HTTP, no libp2p — tested in isolation (shared with #3)
    balance.rs
    transfer.rs
  api/               # localhost HTTP — axum Routes → service calls (ticket #3)
    mod.rs
  mcp/               # optional Phase-2 MCP adapter over same service (ticket #3)
  bin/ai-bank.rs     # clap CLI: `ai-bank serve` (spawn swarm + localhost HTTP), `ai-bank peers add --alias`
```

#### 8.2 MVP flow (behind NAT, no cloud)

1. **First run:** `Keypair::generate_ed25519()` → `to_protobuf_encoding()` → `~/.ai-bank/identity.key` (`0o600`). `PeerId = keypair.public().to_peer_id()`. Also write chosen `127.0.0.1` port for local API.
2. **Listen:** `swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?` + same for `udp/0/quic-v1`. Report via `SwarmEvent::NewListenAddr { address }`.
3. **Bootstrap (one known relay):** `Multiaddr::from_str("/ip4/RELAY_IP/tcp/4001/p2p/RELAY_PEER_ID")` from config. `swarm.dial(addr)?`. `identify` learns relay's `observed_addr`. `autonat` probes reachability; if `Private`, client does `relay_client:reserve` (`Hop RESERVE` → `STATUS OK { expire, addrs, voucher, limit }`).
4. **Discovery:** On each `identify::Event::Received`, for every `listen_addrs` call `kad.add_address(&peer_id, addr)`. On LAN, `mdns::Event::Discovered(list)` does the same. When enough peers are known, `kad.bootstrap()`; ulterior peers arrive via `KademliaEvent::RoutingUpdated` / `GetClosestPeers`.
5. **Relay-routed dial (two NATed nodes):** Dialer resolves `target = /p2p/<relay>/p2p-circuit/p2p/<targetPeer>` from registry/DHT → dials relay (`hop CONNECT to targetPeer`) → relay opens `stop CONNECT from dialer` to target → both `STATUS OK` → streams bridged → `Noise_XX_25519_ChaChaPoly_SHA256` + `yamux` upgrade on the relayed byte stream (no extra TLS).
6. **Hole-punch upgrade (immediate):** After relay stream is up, `B` opens `/libp2p/dcutr` → `Connect{ObsAddrs}` (addresses learned from `identify`) → RTT measured → `Sync` + `RTT/2` wait → simultaneous dial on each `ObsAddrs` (TCP simultaneous-open / QUIC spray 10–200 ms). First success → `A` cancels other attempts, peers migrate new `gossipsub`/`request-response` streams to the direct connection, close relay after grace. Retry up to 3× if peer learned new `observed_addr`.
7. **App protocols:** Over the upgraded connection, `request_response` handles `POST /transfer` analogue (`TransferReq { from: PeerId, to: PeerId, amount: u64, nonce, sig }` → `TransferRes { tx_id, status }`), and `gossipsub` gossips confirmed txs to the topic for ledger convergence (unsigned messages are dropped + peer-score penalized). Discovery for `/ai-bank/registry` also goes via `kad` `get_record`/`put_record`.
8. **Local plane stays unchanged:** agent still does `POST http://127.0.0.1:<port>/v1/transfer {to, amount}` → service layer validates and signs via the same `Keypair` → then submits to swarm's gossip/rpc path. Service layer does not know which transport delivered the transfer — P2P handler and localhost handler both call it.

#### 8.3 When to add gRPC / WebSocket

- **Need typed streaming RPCs or want protobuf contracts?** Add `libp2p-grpc-rs` as a `NetworkBehaviour` and expose `.proto` methods over yamux. Agents still see HTTP locally — only inter-node upgrades.
- **Need browser-native nodes or dashboard pubsub?** Add `libp2p-websocket` / `libp2p-webtransport-websys` transports and a `GET /events` SSE (from ticket #3) bridged to `gossipsub` via the local API. Do not replace the core transport stack.

#### 8.4 Phases (so ordering is clear)

**Phase 0 (now — this ticket closes here):** decision + `SwarmBuilder` skeleton + `identify`+`ping` smoke test between two public nodes (no NAT) over TCP+Noise+Yamux.

**Phase 1 (MVP ledger):** add `kad` (server mode) + `autonat`+`relay`+`dcutr`+`mdns` + `gossipsub` or `request-response` for transfers; deliver "two home-LAN NATed nodes can gossip a transfer via relay with automatic hole-punch upgrade where possible".

**Phase 2 (polish):** `quic` + `upnp` hints, `dns` wrapper, CERTHASH-style bootstrap for WebTransport, ledger-specific `kad` provider records vs value records tuning, peer-score, reservation-voucher persistence across restarts, optional `rendezvous` registry for smaller networks.

**Phase 3 (only if schema pressure demands):** `tonic` + `libp2p-grpc-rs` overlay for typed bidi streams; still inside the libp2p connection.

#### 8.5 Open questions to resolve with other tickets

- **#2 (Node identity) — rotation/lossUX:** Identity key loss = identity loss until a signed `old→new` rotation statement exists; document backup guidance (`identity.key` export). Complement `peers.json` petnames with shared signed `alias` hints in `kad` once registry exists.
- **#3 (Agent interface) — plane separation:** Keep `127.0.0.1:<port>` for agent→node (HTTP+optional MCP). Inter-node `Swarm::listen_on("0.0.0.0:…")` is public — do not reuse the localhost port. The `service` layer should be transport-blind.
- **Shared registry / ledger replication (#5ff):** `kad` DHT semantics (replication factor 20, `Quorum::One` vs `N`, write-back caching) govern how transfers persist without a cloud DB. `gossipsub` mesh degree vs privacy is its own safety consideration.

---

## 9. Common Failure Modes (so they get avoided by design)

| Anti-pattern | Why it hurts | Correct pattern |
|---|---|---|
| "Add `libp2p-tcp` but forget `libp2p-dns`" | DHT/relay addresses that include `/dnsaddr/...` fail to resolve; swarm cannot bootstrap. | Put `.with_dns()` before relay in `SwarmBuilder`, or via `libp2p-dns` as a wrapper transport around TCP. Builder in 0.55+ enforces ordering. |
| "Add `kad` but not `identify`" | Rust libp2p does not auto-wire Identify into Kademlia — `add_address` is never called → `RoutingUpdated` stays at 1 peer (bootnode) and no further discovery. | On `identify::Event::Received { peer_id, info }`, loop `info.listen_addrs` → `kad.add_address(&peer_id, addr)`. Treat as required glue. |
| "Expect HTTP gRPC proxy to traverse NAT" | Paying for an Envoy/ILB still needs public-IP reachability on one side; does not help two home NATs connecting to each other. | Use libp2p relay + DCUtR; reserve only when `AutoNAT::Private`. |
| "Treat `PeerId` as just a header string" | No handshake proof → any attacker can claim `alice (12D3KooA)` and serve fake HTTP on any IP. | Let Noise or TLS verify `PeerId` in the encrypted handshake, and sign app payloads with `b"/ai-bank/1/…"`. Header-only verification invites spoofing. |
| "Use WebSocket as *the* inter-node protocol instead of as a transport inside libp2p" | Still no discovery / hole-punch; every node needs a public `wss://` endpoint + CA. | Use `libp2p-websocket` as an *additional transport* inside the swarm if you need browser reachability, not as a protocol. |
| "Stay relay-only forever" | Relays are capped (`Limit{duration,data}`) — long queries exceed cap and get reset; bandwidth bottlenecks. | Attempt DCUtR immediately after relay establishment; keep relay as signalling + fallback only. |

---

## Appendix: Primary Sources

- `libp2p/specs/peer-ids/peer-ids.md` — Keys (deterministic protobuf `PublicKey{Type,Data}`, required fields, supported types `RSA`/`Ed25519`/`Secp256k1`/`ECDSA`), PeerIds (hashing rule ≤42 B identity vs >42 B sha2-256, base58btc legacy / CIDv1 `libp2p-key` base32, verification `is_public_key`). [Raw spec](https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md)
- `libp2p/specs/noise/README.md` — Noise-libp2p XX handshake `Noise_XX_25519_ChaChaPoly_SHA256`, separate X25519 static key authenticated via payload `identity_key`/`identity_sig` with domain `noise-libp2p-static-key:`, must-verify-then-use signature before encrypted transport. [Spec](https://github.com/libp2p/specs/blob/master/noise/README.md)
- `libp2p/specs/relay/circuit-v2.md` — Relay v2 `hop` `/libp2p/circuit/relay/0.2.0/hop` (+ `stop` `/libp2p/circuit/relay/0.2.0/stop`), `HopMessage { RESERVE\|CONNECT\|STATUS }`, `Reservation{expire,addrs,voucher}`, `Limit{duration,data}`, voucher `SignedEnvelope` domain `libp2p-relay-rsvp` (multicodec `0x0302`) payload `{relay,peer,expiration}`, MUST-NOT-reserve-over-relay. [Spec](https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md)
- `libp2p/specs/relay/DCUtR.md` — Direct Connection Upgrade through Relay: relay connection → `/libp2p/dcutr` → `HolePunch { CONNECT { ObsAddrs } }` + RTT measurement → `SYNC` + `RTT/2` wait → simultaneous dial (TCP simultaneous-open, QUIC random-bytes spray 10–200 ms), `ObsAddrs` as binary multiaddrs, 4 KiB cap, retry twice. [Spec](https://github.com/libp2p/specs/blob/master/relay/DCUtR.md)
- `libp2p/specs/discovery/mdns.md` / `libp2p.io/docs/mdns/` — mDNS service `_p2p._udp.local PTR`, `TXT dnsaddr=…/p2p/<PeerId>` (+ `SRV`/`A`/`AAAA` additional records), 32+ char random `peer-name` not PeerId (DNS label limit), loopback/NAT-busting addresses ignored. [Spec](https://github.com/libp2p/specs/blob/master/discovery/mdns.md), [Docs](https://libp2p.io/docs/mdns/)
- `docs.rs/libp2p_kad` — Kademlia-identify discrepancy note ("Rust-libp2p tries to stay as generic as possible … Identify protocol must be manually hooked up through `add_address`"). `libp2p/rust-libp2p` issues #2673 / discussion `5357` (DCUtR vs relay vs rendezvous vs autonat explainer). [KAD docs](https://docs.rs/libp2p-kad/latest/libp2p_kad/), [Discussion 5357](https://github.com/libp2p/rust-libp2p/discussions/5357)
- `libp2p/io` concepts — `Dcutr` / `AutoNAT` / `Relay` / `Identify` / `Kademlia` / `Connections`; `WebRTC`/`WebTransport` browser notes (STUN/TURN distribution, cert-hash in multiaddr + Noise-on-first-stream vs `wss` 5 RTT). [Docs](https://libp2p.io/docs/dcutr/)
- `crates.io — libp2p 0.54.1` / `0.55.0` / `0.56.0`, and `docs.rs/crate/libp2p/0.55.0/features` — feature flags `tokio`/`tcp`/`quic`/`dns`/`noise`/`tls`/`yamux`/`identify`/`kad`/`autonat`/`relay`/`dcutr`/`mdns`/`gossipsub`/`request-response`/`upnp`/`webtransport-websys`/`websocket-websys`, MSRV `1.75.0` (0.54.1) / `1.83.0` (0.55/0.56), download counts (0.54.1: 4M+). [Crate](https://crates.io/crates/libp2p/0.55.0), [Features](https://docs.rs/crate/libp2p/0.55.0/features)
- `rust-libp2p releases — SwarmBuilder example (0.55.0)` — type-safe builder `with_new_identity().with_tokio().with_tcp(...).with_quic().with_dns().with_relay_client(...).with_behaviour(|keypair, relay_client| … dcutr::Behaviour::new(peer_id))`. [Release notes](https://github.com/libp2p/rust-libp2p/releases)
- `crates.io — axum 0.8.9` / `docs.rs/crate/axum 0.8.9` — routing `tower::Service`/`tower-http`, hyper 1.4, `forbid(unsafe_code)`, MSRV `1.80`. [Crate](https://crates.io/crates/axum/0.8.9), [Docs](https://docs.rs/crate/axum/0.8.9)
- `docs.rs tonic` / `crates.io tonic 0.12.3` — gRPC over HTTP/2, `transport` (`server`/`channel`) built on hyper+tower+tokio, `router` feature `Routes::into_axum_router()`, `tonic-build`/`prost` codegen, `tls` via rustls, `GrpcWebLayer` needing `accept_http1(true)` on cleartext. [Docs](https://docs.rs/tonic/latest/tonic/)
- `libp2p-grpc-rs` (community) — `NetworkBehaviour` + `DirectGrpcUpgrade` lets tonic `Server`/`Channel` run over libp2p yamux streams; transport-agnostic (TCP/QUIC/WebRTC) with Noise; integrates peer discovery/NAT from swarm. [DeepWiki](https://deepwiki.com/0xbillw/libp2p-grpc-rs)
- `Pilot Protocol — NATS vs gRPC vs TCP vs Pilot` comparison — Table notes: "TCP and gRPC have zero NAT traversal capability … NATS sidesteps via broker … Pilot is only one … that treats NAT as a first-class problem" (quotes used for the gRPC/behind-NAT claim above). [Blog](https://pilotprotocol.network/blog/pilot-vs-tcp-grpc-nats-comparison)
- `ark-builders — P2P: WebRTC vs libp2p vs Iroh` — libp2p as Swiss-army-knife vs iroh's "limited centralization to make things easier" and `webrtc` STUN/TURN dependency; framing as complexity vs NAT handling trade-off. [Medium](https://ark-builders.medium.com/the-deceptive-complexity-of-p2p-connections-and-the-solution-we-found-d2b5cbeddbaf)
- Branch `research/node-identity` — `docs/research/node-identity.md` (commit `047a67e`) — Ed25519 `Keypair::generate_ed25519()` → `to_protobuf_encoding()` → `from_protobuf_encoding`, `PeerId::is_public_key`, Noise/TLS binding, petname table, signed peer records `CertifiedAddrBook`. Context pointer for §6.
- Branch `research/agent-interface` — `docs/research/agent-interface.md` (commit `eaa145f`) — agent↔node locus is `axum` on `127.0.0.1` with MCP as thin adapter; distinct from node↔node — motivates the two-plane recommendation of this doc.

---

*Next step: Decision-maker reviews §8 and records ADR `docs/adr/0002-communication-protocol.md` locking in libp2p swarm for node↔node (Noise/TLS+Yamux+QUIC/TCP+identify+kad+autonat+relay+dcutr) with axum on localhost retained for agent↔node. Ticket #4 can then be closed with a pointer to this file. Dependency on #2: if #2 is adopted, §6 holds; if #2 revisions change KeyType (e.g. add RSA), §4.1 PeerId encoding note requires a companion migration ADR.*

# ADR 0001: Node identity is Ed25519 PeerId with petname overlay

Nodes generate an Ed25519 keypair locally on first run and derive a libp2p `PeerId` (`12D3Koo…`) as the canonical identity. The public key is the identity; no central CA or registry issues it. Transport authenticity is provided by Noise XX (or TLS 1.3) handshake verification of `PeerId::is_public_key`. Human-readable names are a local petname table (`PeerId → alias`), with optional shared signed `PeerRecord` hints, never the source of truth — see `research/node-identity` decision record.

## Status

Accepted — implements wayfinder ticket [#2 Node identity: how nodes prove who they are](https://github.com/kudosscience/ai-bank/issues/2) → `research/node-identity` (`docs/research/node-identity.md`).

## Context

AI Bank runs on users' own machines with no cloud bills, so identity must be self-sovereign and verifiable offline. Pure human-readable names require a central naming authority (Zooko's Triangle), violating the decentralisation constraint. The choice blocks communication protocol, registry, ledger binding, and reputation.

## Considered Options

- **Ed25519 keypair → PeerId (chosen)** — 32B key, multihashed `PeerId`, `libp2p-identity 0.2` / `ed25519-dalek`, file `~/.ai-bank/identity.key` (`to_protobuf_encoding`), `sign`/`verify` with domain `b"/ai-bank/1/"`, handshake-bound.
- **Human-readable names only (rejected)** — global uniqueness needs a hosted registry; squatting/spoofing, operator trust, ongoing cost.
- **Hybrid: keypair + petname (chosen as overlay)** — local `peers.json` alias map like phone contacts (Spritely petnames paper), optional DHT/gossip `SignedEnvelope` hints via `CertifiedAddrBook`. Display `alias (short PeerId)`, ledger writes use `PeerId`.

Key-type scope locked to Ed25519-only for MVP; RSA/Secp256k1, W3C DID, and username/password rejected (size/complexity, premature, replayable).

## Consequences

- Dependency: communication protocol remains libp2p-compatible (Noise preferred, TLS alternative) — #4.
- Persistence is operator's responsibility: lose `identity.key` → lose `PeerId`; provide `export`/`import` and document.
- Rotation = new `PeerId`; continuity via signed `old→new` rotation statement if needed.
- Reputation keys off `PeerId`, not alias; UI must not show unverified aliases alone (phishing mitigation).
- Implementation crates: `libp2p-identity` (`ed25519`, `peerid`, `rand`), `multihash 0.19`, `bs58 0.5`, `libp2p-noise`; interop note: don't cast `ed25519_dalek::SigningKey` to `libp2p_identity::Keypair`—round-trip via bytes/protobuf.

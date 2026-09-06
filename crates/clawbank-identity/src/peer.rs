//! Canonical PeerId text forms (ADR-0001): base58 plus CIDv1/base32.

use libp2p_identity::{Keypair, PeerId};
use std::io;

/// The canonical identity for a keypair: the PeerId derived from its public key.
pub fn peer_id(keypair: &Keypair) -> PeerId {
    keypair.public().to_peer_id()
}

/// The PeerId in base58 text form (`12D3Koo...`).
pub fn peer_id_base58(id: &PeerId) -> String {
    id.to_base58()
}

/// The PeerId in CIDv1 form (`bafz...`): multibase-base32 of the
/// version + libp2p-key codec + multihash bytes.
pub fn peer_id_cid(id: &PeerId) -> String {
    let mut raw = vec![0x01u8, 0x72u8];
    raw.extend_from_slice(&id.to_bytes());
    format!(
        "b{}",
        data_encoding::BASE32_NOPAD.encode(&raw).to_lowercase()
    )
}

/// Parse the CID form produced by [`peer_id_cid`].
pub fn peer_id_from_cid(text: &str) -> io::Result<PeerId> {
    let invalid: fn() -> io::Error =
        || io::Error::new(io::ErrorKind::InvalidData, "not a peer CID");
    let body = text.strip_prefix('b').ok_or_else(invalid)?;
    let raw = data_encoding::BASE32_NOPAD
        .decode(body.to_uppercase().as_bytes())
        .map_err(|_| invalid())?;
    if raw.len() < 2 || raw[0] != 0x01 || raw[1] != 0x72 {
        return Err(invalid());
    }
    PeerId::from_bytes(&raw[2..]).map_err(|_| invalid())
}

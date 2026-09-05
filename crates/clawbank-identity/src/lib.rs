//! Node identity: Ed25519 keypair to libp2p PeerId (ADR-0001).
//!
//! The public key is the identity. Keys are generated locally, persisted
//! as protobuf, and never leave the machine.

use libp2p_identity::{Keypair, PeerId};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Generate a fresh Ed25519 node keypair from the OS random source.
pub fn generate() -> Keypair {
    Keypair::generate_ed25519()
}

/// The canonical identity for a keypair: the PeerId derived from its public key.
pub fn peer_id(keypair: &Keypair) -> PeerId {
    keypair.public().to_peer_id()
}

/// The PeerId in base58 text form (`12D3Koo...`).
pub fn peer_id_base58(id: &PeerId) -> String {
    id.to_base58()
}

/// Write bytes with owner-only permissions on Unix (profile ACLs elsewhere).
fn write_secure(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
        fs::set_permissions(
            path.parent().unwrap_or(Path::new(".")),
            fs::Permissions::from_mode(0o700),
        )?;
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true).mode(0o600);
        io::Write::write_all(&mut opts.open(path)?, bytes)
    }
    #[cfg(not(unix))]
    {
        // Windows has no Unix permission bits; the file inherits the
        // user's profile ACLs. Documented limitation, not silent: callers
        // on shared machines should prefer an encrypted volume.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)
    }
}

/// Persist a keypair as protobuf. See [`write_secure`] for permissions.
pub fn save(keypair: &Keypair, path: &Path) -> io::Result<()> {
    let bytes = keypair
        .to_protobuf_encoding()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_secure(path, &bytes)
}

/// Load a keypair persisted with [`save`]. A missing file is an error;
/// see [`load_or_generate`] for first-run behavior.
pub fn load(path: &Path) -> io::Result<Keypair> {
    let bytes = fs::read(path)?;
    Keypair::from_protobuf_encoding(&bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Load the identity at `path`, generating and persisting a fresh one
/// when no file exists yet. Generation failures and corrupt files are
/// errors; only a missing file triggers generation.
pub fn load_or_generate(path: &Path) -> io::Result<Keypair> {
    match load(path) {
        Ok(keypair) => Ok(keypair),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let keypair = generate();
            save(&keypair, path)?;
            Ok(keypair)
        }
        Err(e) => Err(e),
    }
}

fn home_dir() -> io::Result<PathBuf> {
    #[cfg(windows)]
    let home: Option<PathBuf> = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            let drive = std::env::var("HOMEDRIVE").ok()?;
            let path = std::env::var("HOMEPATH").ok()?;
            Some(PathBuf::from(format!("{drive}{path}")))
        });
    #[cfg(windows)]
    return home
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is not set"));
    #[cfg(unix)]
    return std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "home directory is not set"));
}

/// The node data directory: `$CLAWBANK_HOME` when set (operators and
/// tests), otherwise a `.clawbank` folder under the user's home.
pub fn data_dir() -> io::Result<PathBuf> {
    match std::env::var("CLAWBANK_HOME") {
        Ok(dir) => Ok(PathBuf::from(dir)),
        Err(_) => home_dir().map(|home| home.join(".clawbank")),
    }
}

/// The identity file inside [`data_dir`].
pub fn identity_file() -> io::Result<PathBuf> {
    data_dir().map(|dir| dir.join("identity.key"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_keypairs_have_distinct_peer_ids() {
        let a = generate();
        let b = generate();
        assert_ne!(peer_id(&a), peer_id(&b));
    }

    #[test]
    fn saved_keypair_reloads_with_identical_peer_id() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("identity.key");
        let original = generate();
        save(&original, &file).unwrap();
        let loaded = load(&file).unwrap();
        assert_eq!(peer_id(&original), peer_id(&loaded));
    }

    #[test]
    #[cfg(unix)]
    fn saved_identity_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("identity.key");
        save(&generate(), &file).unwrap();
        let file_mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "identity file must be owner-only");
        let dir_mode = std::fs::metadata(file.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "identity dir must be owner-only");
    }

    #[test]
    fn missing_file_load_or_generate_creates_then_reloads_same_identity() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("identity.key");
        let first = load_or_generate(&file).unwrap();
        assert!(file.exists());
        let second = load_or_generate(&file).unwrap();
        assert_eq!(peer_id(&first), peer_id(&second));
    }

    #[test]
    fn peer_id_cid_round_trips_and_embeds_multihash_bytes() {
        let id = peer_id(&generate());
        let cid = peer_id_cid(&id);
        assert!(cid.starts_with('b'), "CID form must use multibase base32");
        assert_eq!(peer_id_from_cid(&cid).unwrap(), id);
        let raw = data_encoding::BASE32_NOPAD
            .decode(cid[1..].to_uppercase().as_bytes())
            .unwrap();
        assert_eq!(&raw[2..], &id.to_bytes()[..]);
    }

    #[test]
    fn data_dir_defaults_to_dot_clawbank_under_home() {
        let dir = data_dir().unwrap();
        assert_eq!(dir.file_name().unwrap().to_str().unwrap(), ".clawbank");
    }

    #[test]
    fn peer_id_base58_round_trips_through_text() {
        let id = peer_id(&generate());
        let text = peer_id_base58(&id);
        let parsed: PeerId = text.parse().unwrap();
        assert_eq!(id, parsed);
    }
}

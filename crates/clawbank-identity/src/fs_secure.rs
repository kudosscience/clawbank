//! Owner-only, atomic file writes and the directories around them.
//!
//! Unix is the hardened path: exclusive-create temp files, explicit `0600`,
//! atomic publish by rename (which replaces a destination symlink instead
//! of following it), and `0700` only for directories we create ourselves.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Sibling of `path` with a dotted suffix, e.g. `.identity.key.lock`.
/// Siblings stay on the same filesystem so `rename` publish is atomic.
pub(crate) fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "identity.key".to_string());
    let sibling = format!(".{name}.{suffix}");
    match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(dir) => dir.join(sibling),
        None => PathBuf::from(sibling),
    }
}

/// Create the parent chain of `path`, `0700`-ing only directories that did
/// not exist yet. Pre-existing (operator-owned) parents keep their mode,
/// and the filesystem root is refused outright: it is never ours to chmod.
pub(crate) fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        // Parentless paths resolve against the CWD, which we must not manage.
        return Ok(());
    }
    #[cfg(unix)]
    {
        if parent == Path::new("/") {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "refusing to manage the filesystem root",
            ));
        }
        create_dir_only_missing(parent)?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(parent)?;
    Ok(())
}

/// True when `path` is a regular file whose Unix mode is not owner-only.
/// Symlinks and non-regular files are left alone: republishing through
/// them would replace the link itself.
#[cfg(unix)]
pub(crate) fn needs_perm_repair(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => meta.permissions().mode() & 0o777 != 0o600,
        _ => false,
    }
}

/// No permission bits to repair off Unix.
#[cfg(not(unix))]
pub(crate) fn needs_perm_repair(_path: &Path) -> bool {
    false
}

/// fsync the containing directory so a just-published rename is durable,
/// not merely ordered. Skipped for parentless paths (nothing to sync).
#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(dir) => fs::File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}
/// Create `dir`, `0700`-ing only the chain members that were missing.
#[cfg(unix)]
fn create_dir_only_missing(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut missing = Vec::new();
    let mut cursor = Some(dir);
    while let Some(component) = cursor {
        if component.as_os_str().is_empty() || component == Path::new("/") {
            break;
        }
        if component.exists() {
            break;
        }
        missing.push(component.to_path_buf());
        cursor = component.parent();
    }
    fs::create_dir_all(dir)?;
    for component in missing {
        if component != Path::new("/") {
            fs::set_permissions(&component, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Write bytes with owner-only permissions on Unix (profile ACLs elsewhere).
///
/// The write is atomic: bytes land in an exclusively-created sibling temp
/// file and are renamed over the target, so readers never see a truncated
/// identity and a crash can only leave a complete file or none. The parent
/// directory is synced after the rename so the publish survives power loss.
/// Stale sibling temp files after a crash are safe to delete.
pub(crate) fn write_secure(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        ensure_parent_dir(path)?;
        let pid = std::process::id();
        let mut attempt = 0u32;
        loop {
            let tmp = sibling_path(path, &format!("tmp.{pid}.{attempt}"));
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create_new(true).mode(0o600);
            match opts.open(&tmp) {
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    attempt = attempt.saturating_add(1);
                    if attempt > 100 {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "identity temp files exhausted",
                        ));
                    }
                }
                Err(e) => return Err(e),
                Ok(mut file) => {
                    // `mode(0o600)` only applies at creation; enforce it so
                    // re-saving over a loosened file tightens it back.
                    let outcome = (|| -> io::Result<()> {
                        file.set_permissions(fs::Permissions::from_mode(0o600))?;
                        io::Write::write_all(&mut file, bytes)?;
                        file.sync_all()?;
                        drop(file);
                        fs::rename(&tmp, path)?;
                        sync_parent(path)
                    })();
                    if outcome.is_err() {
                        let _ = fs::remove_file(&tmp);
                    }
                    return outcome;
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        // Windows has no Unix permission bits; the file inherits the
        // user's profile ACLs. Documented limitation, not silent: callers
        // on shared machines should prefer an encrypted volume.
        //
        // Same temp+publish shape as Unix, but std has no atomic replace
        // on Windows: when the destination exists we remove-then-rename.
        // A crash can then leave the file missing (regenerated, with a new
        // PeerId, on next init) but never truncated (which would hard-error
        // every later init instead of recovering).
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let pid = std::process::id();
        let mut attempt = 0u32;
        loop {
            let tmp = sibling_path(path, &format!("tmp.{pid}.{attempt}"));
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create_new(true);
            match opts.open(&tmp) {
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    attempt = attempt.saturating_add(1);
                    if attempt > 100 {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "identity temp files exhausted",
                        ));
                    }
                }
                Err(e) => return Err(e),
                Ok(mut file) => {
                    let outcome = (|| -> io::Result<()> {
                        io::Write::write_all(&mut file, bytes)?;
                        file.sync_all()?;
                        drop(file);
                        match fs::rename(&tmp, path) {
                            Ok(()) => Ok(()),
                            // Replace only when the rename lost a
                            // create-vs-replace race: any other failure
                            // must propagate with the good file intact.
                            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && path.exists() => {
                                fs::remove_file(path)?;
                                fs::rename(&tmp, path)
                            }
                            Err(e) => Err(e),
                        }
                    })();
                    if outcome.is_err() {
                        let _ = fs::remove_file(&tmp);
                    }
                    return outcome;
                }
            }
        }
    }
}

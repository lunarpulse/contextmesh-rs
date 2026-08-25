//! OC-01 bounded regular-file import/export (Stage 2E).
//!
//! Export re-verifies a ledger, renders canonical JCS bytes, refuses an
//! existing destination, writes all bytes to a newly created regular file,
//! syncs it, and removes its partial new file on write/sync failure. It never
//! writes the Option A database. Import accepts only regular non-symlink
//! files, reads at most `max_wire_bytes + 1`, rejects excess, and calls
//! `from_wire`. Verified import additionally performs DAG and current-snapshot
//! verification before returning. No import sanitizes, rewrites, sorts, or
//! repairs input.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::{OutcomeOperationError, OutcomeOperationResult};
use crate::outcome::SignedOutcomeLedgerV1;
use crate::types::OutcomeLimits;

/// Upper bound for one bounded read chunk.
const READ_CHUNK: usize = 8192;

/// Writes a fully verified ledger as canonical JCS bytes to a new regular
/// file at `path` and syncs it.
///
/// The destination must not already exist: creation uses create-new
/// semantics and never truncates an existing path. On any write or sync
/// failure the partial new file is removed (best-effort unlink) and no
/// success is returned. The Option A database is never written.
///
/// # Errors
/// Fails closed with [`OutcomeOperationError`]; on write/sync failure it
/// attempts to remove its partial new file before returning.
pub fn export_outcome(
    ledger: &SignedOutcomeLedgerV1,
    path: &Path,
    limits: OutcomeLimits,
) -> OutcomeOperationResult<()> {
    // Re-verify first: structural verify covers canonicality, ID derivation,
    // and the domain signature before any byte reaches the filesystem.
    ledger.verify(limits)?;
    let wire = ledger.to_wire(limits)?;

    // Create-new refuses an existing destination without truncating it.
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(OutcomeOperationError::Io)?;
    write_sync_or_cleanup(path, file, &wire, |opened| opened.sync_all())
        .map_err(OutcomeOperationError::Io)
}

/// Reads and strictly parses a ledger from a regular non-symlink file.
///
/// Reads at most `max_wire_bytes + 1` bytes; a file longer than
/// `max_wire_bytes` is rejected by `from_wire` as `limit-exceeded`.
///
/// # Errors
/// Fails closed with [`OutcomeOperationError`]; never returns a partial
/// ledger.
pub fn import_outcome(
    path: &Path,
    limits: OutcomeLimits,
) -> OutcomeOperationResult<SignedOutcomeLedgerV1> {
    let bytes = read_bounded(path, limits.max_wire_bytes + 1)?;
    SignedOutcomeLedgerV1::from_wire(&bytes, limits).map_err(OutcomeOperationError::Artifact)
}

/// Reads, strictly parses, and then verifies the ledger against the admitted
/// DAG and the current ref snapshot before returning it.
///
/// # Errors
/// Fails closed with [`OutcomeOperationError`] on structural, DAG, or
/// freshness failure; never returns a partial ledger.
pub async fn import_outcome_verified(
    path: &Path,
    store: &contextmesh::store::Store,
    limits: OutcomeLimits,
) -> OutcomeOperationResult<SignedOutcomeLedgerV1> {
    let ledger = import_outcome(path, limits)?;
    ledger.verify_current_inputs(store, limits).await?;
    Ok(ledger)
}

/// Writes all bytes, syncs, and on write/sync failure removes only the
/// partial new file at `path`.
///
/// The writer and sync hooks are generic so the failure-cleanup branch stays
/// executable under injected faults in tests; production passes the opened
/// file and [`File::sync_all`].
fn write_sync_or_cleanup<W: Write, F>(
    path: &Path,
    mut writer: W,
    wire: &[u8],
    sync: F,
) -> std::io::Result<()>
where
    F: FnOnce(&mut W) -> std::io::Result<()>,
{
    let outcome = writer.write_all(wire).and_then(|()| sync(&mut writer));
    if let Err(error) = outcome {
        // Failure cleanup: close the writer first, then remove only the
        // partial new file we created.
        drop(writer);
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

/// Reads at most `limit` bytes from a regular non-symlink file.
///
/// A file longer than the bound still yields exactly its first `limit` bytes
/// so the caller's strict size check reports `limit-exceeded` exactly.
fn read_bounded(path: &Path, limit: usize) -> OutcomeOperationResult<Vec<u8>> {
    // symlink_metadata inspects the link itself: a symlink to a regular
    // file reports is_file() == false, so symlinks reject here.
    let metadata = std::fs::symlink_metadata(path).map_err(OutcomeOperationError::Io)?;
    if !metadata.is_file() {
        return Err(OutcomeOperationError::Io(std::io::Error::other(
            "not a regular file",
        )));
    }
    let mut file = File::open(path).map_err(OutcomeOperationError::Io)?;
    let mut bytes = Vec::with_capacity(metadata.len().min(limit as u64) as usize);
    let mut chunk = vec![0u8; READ_CHUNK];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => return Ok(bytes),
            Ok(n) => {
                if bytes.len() + n > limit {
                    bytes.extend_from_slice(&chunk[..limit - bytes.len()]);
                    return Ok(bytes);
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(OutcomeOperationError::Io(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OC01-X02 (injected write failure): a writer that fails part-way
    /// through leaves no destination file.
    #[test]
    fn write_failure_removes_partial_new_file() {
        let path = std::env::temp_dir().join(format!(
            "oc01-io-unit-write-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        // The file our create-new step would have opened already exists on
        // disk; the injected writer then fails part-way.
        std::fs::write(&path, b"").expect("partial new file stages");

        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "injected short write",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let result = write_sync_or_cleanup(&path, FailingWriter, b"canonical bytes", |_| Ok(()));
        assert!(result.is_err(), "injected write failure must fail");
        assert!(
            !path.exists(),
            "partial new file must be removed on write failure"
        );
    }

    /// OC01-X02 (injected sync failure): a successful write followed by a
    /// failing sync still leaves no destination file.
    #[test]
    fn sync_failure_removes_partial_new_file() {
        let path = std::env::temp_dir().join(format!(
            "oc01-io-unit-sync-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, b"").expect("partial new file stages");

        let result = write_sync_or_cleanup(
            &path,
            std::io::Cursor::new(Vec::new()),
            b"canonical bytes",
            |_| Err(std::io::Error::other("injected sync failure")),
        );
        assert!(result.is_err(), "injected sync failure must fail");
        assert!(
            !path.exists(),
            "partial new file must be removed on sync failure"
        );
    }

    /// Shared serial for unit-test scratch paths.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
}

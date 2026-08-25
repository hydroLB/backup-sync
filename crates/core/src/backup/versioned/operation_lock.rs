use crate::hashing;
use crate::io::BlockingIoPolicy;
use anyhow::{Context, Result};
use fs2::FileExt;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const OPERATION_LOCK_FILE: &str = "operation.lock";

/// An exclusive advisory lease for one destination's versioned store.
///
/// Dropping the lease releases the lock. Callers must keep it alive for the
/// complete operation that reads or mutates store metadata.
#[derive(Debug)]
pub(crate) struct StoreOperationLease {
    file: File,
    store_id: String,
}

impl Drop for StoreOperationLease {
    fn drop(&mut self) {
        if let Err(error) = FileExt::unlock(&self.file) {
            tracing::warn!(
                store_id = %self.store_id,
                error = %error,
                "versioned store operation lease failed to unlock; closing the file handle"
            );
        }
    }
}

/// A set of destination leases held in deterministic path order.
#[derive(Debug)]
pub(crate) struct StoreOperationLeases {
    _leases: Vec<StoreOperationLease>,
}

/// Acquire one destination-scoped lease with bounded polling.
pub(crate) fn acquire_store_lease(
    destination_root: &Path,
    policy: &BlockingIoPolicy,
) -> Result<StoreOperationLease> {
    let normalized = validate_and_normalize_destination(destination_root)?;
    acquire_normalized_store_lease(&normalized, policy)
}

/// Acquire every unique destination lease in deterministic normalized-path order.
///
/// This ordering prevents two multi-store operations from deadlocking when their
/// input destination order differs.
pub(crate) fn acquire_store_leases<'a>(
    destination_roots: impl IntoIterator<Item = &'a Path>,
    policy: &BlockingIoPolicy,
) -> Result<StoreOperationLeases> {
    let mut destinations = BTreeSet::<PathBuf>::new();
    for destination_root in destination_roots {
        destinations.insert(validate_and_normalize_destination(destination_root)?);
    }

    let mut leases = Vec::with_capacity(destinations.len());
    for destination_root in destinations {
        leases.push(acquire_normalized_store_lease(&destination_root, policy)?);
    }
    Ok(StoreOperationLeases { _leases: leases })
}

fn acquire_normalized_store_lease(
    destination_root: &Path,
    policy: &BlockingIoPolicy,
) -> Result<StoreOperationLease> {
    let store_id = store_id(destination_root);
    let metadata_root = destination_root.join(super::store::STORE_DIR);
    fs::create_dir_all(&metadata_root).with_context(|| {
        format!("versioned store operation lease could not prepare store {store_id}")
    })?;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(metadata_root.join(OPERATION_LOCK_FILE))
        .with_context(|| {
            format!("versioned store operation lease could not open store {store_id}")
        })?;

    let started = Instant::now();
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(StoreOperationLease { file, store_id }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() >= policy.timeout {
                    anyhow::bail!(
                        "versioned store {store_id} is busy; timed out after {:?} waiting for its operation lease; retry after the active backup or maintenance operation finishes",
                        policy.timeout
                    );
                }
                let remaining = policy.timeout.saturating_sub(started.elapsed());
                std::thread::sleep(
                    remaining.min(
                        policy
                            .backoff_poll_interval
                            .max(std::time::Duration::from_millis(1)),
                    ),
                );
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("versioned store operation lease failed for store {store_id}")
                });
            }
        }
    }
}

fn validate_and_normalize_destination(destination_root: &Path) -> Result<PathBuf> {
    let absolute = if destination_root.is_absolute() {
        destination_root.to_path_buf()
    } else {
        std::env::current_dir()
            .context("versioned store operation lease could not resolve the working directory")?
            .join(destination_root)
    };
    let absolute = lexically_normalize(&absolute);

    let metadata = fs::metadata(&absolute).with_context(|| {
        "versioned store destination is unavailable; reconnect or create the configured destination before retrying"
    })?;
    if !metadata.is_dir() {
        anyhow::bail!(
            "versioned store destination is not a directory; select an existing directory before retrying"
        );
    }
    fs::canonicalize(&absolute)
        .context("versioned store operation lease could not canonicalize destination directory")
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn store_id(destination_root: &Path) -> String {
    let digest = hashing::sha256_hex(destination_root.to_string_lossy().as_bytes());
    digest[..12].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    fn short_policy() -> BlockingIoPolicy {
        BlockingIoPolicy {
            timeout: Duration::from_millis(40),
            retry_delays: Vec::new(),
            backoff_poll_interval: Duration::from_millis(5),
        }
    }

    #[test]
    fn same_thread_open_handles_contend_until_the_lease_is_dropped() {
        let temp = tempdir().expect("operation_lock test temp directory");
        let destination = temp.path().join("destination");
        fs::create_dir_all(&destination).expect("operation_lock test destination");
        let policy = short_policy();

        let first = acquire_store_lease(&destination, &policy).expect("first lease");
        let error = acquire_store_lease(&destination, &policy)
            .expect_err("a second open handle in this process must contend");
        assert!(error.to_string().contains("is busy"));
        assert!(!error
            .to_string()
            .contains(&destination.display().to_string()));

        drop(first);
        acquire_store_lease(&destination, &policy).expect("lease after RAII unlock");
    }

    #[test]
    fn multi_store_acquisition_normalizes_and_deduplicates_paths() {
        let temp = tempdir().expect("operation_lock test temp directory");
        let destination = temp.path().join("destination");
        fs::create_dir_all(&destination).expect("operation_lock test destination");
        let alias = destination.join("child").join("..");
        let policy = short_policy();

        let leases = acquire_store_leases([alias.as_path(), destination.as_path()], &policy)
            .expect("normalized lease set");
        assert_eq!(leases._leases.len(), 1);
    }

    #[test]
    fn missing_destination_is_not_created() {
        let temp = tempdir().expect("operation_lock test temp directory");
        let destination = temp.path().join("disconnected-destination");

        let error = acquire_store_lease(&destination, &short_policy())
            .expect_err("missing destination must be rejected");
        assert!(error.to_string().contains("destination is unavailable"));
        assert!(!destination.exists());
    }

    #[test]
    fn multi_store_preflight_rejects_before_creating_any_lock_directory() {
        let temp = tempdir().expect("operation_lock test temp directory");
        let available = temp.path().join("available");
        let missing = temp.path().join("missing");
        fs::create_dir(&available).expect("available destination");

        acquire_store_leases([available.as_path(), missing.as_path()], &short_policy())
            .expect_err("one missing destination must reject the whole acquisition");

        assert!(!available.join(super::super::store::STORE_DIR).exists());
        assert!(!missing.exists());
    }

    #[test]
    fn non_directory_destination_is_rejected_without_store_creation() {
        let temp = tempdir().expect("operation_lock test temp directory");
        let destination = temp.path().join("destination-file");
        fs::write(&destination, b"not a directory").expect("destination file");

        let error = acquire_store_lease(&destination, &short_policy())
            .expect_err("file destination must be rejected");
        assert!(error.to_string().contains("is not a directory"));
        assert!(destination.is_file());
    }
}

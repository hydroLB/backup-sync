pub mod model;
pub mod replication;
pub mod restore;
pub mod scrub;
pub mod store;

pub use model::{Manifest, ManifestEntry, ManifestEntryKind, VersionIndex, VersionInfo};
pub use replication::{replicate_configured_stores, ReplicationSummary};
pub use restore::{
    list_version_files, list_versions, restore_files, restore_version, ListVersionFilesResult,
    RestoreFilesRequest, RestoreMode, RestoreRequest, RestoreResult, VersionFileInfo,
};
pub use scrub::{scrub_versioned_store, ScrubMode, ScrubResult};
pub use store::{
    run_backup_cycle, simulate_backup_cycle, simulate_backup_cycle_with_sample_limit,
    remove_kept_safety_version, BackupCycleResult, FolderBackupResult, FolderDescriptor,
    SimulationSummary,
};

/// Purpose: Return the filesystem path to the versioned store root under a destination directory.
///
/// Inputs: `destination_root` path from config.
/// Outputs: A path like `<destination>/.backup_sync/v1`.
/// Side effects: None.
/// Error handling: Never fails; returns a derived path.
/// Ties to other methods: Used by diagnostics, hardening checks, and store management routines.
/// Why this exists: Callers outside the core store implementation should not re-encode store layout rules.
pub fn store_root_path(destination_root: &std::path::Path) -> std::path::PathBuf {
    store::store_root(destination_root)
}

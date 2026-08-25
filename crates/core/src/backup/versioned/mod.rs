pub mod model;
pub(crate) mod operation_lock;
pub mod replication;
pub mod restore;
pub mod scrub;
pub mod store;
pub(crate) mod validation;

pub use model::{Manifest, ManifestEntry, ManifestEntryKind, VersionIndex, VersionInfo};
pub use replication::{replicate_configured_stores, ReplicationSummary};
pub use restore::{
    list_version_files, list_versions, restore_files, restore_version, ListVersionFilesResult,
    RestoreFilesRequest, RestoreMode, RestoreRequest, RestoreResult, VersionFileInfo,
};
pub use scrub::{scrub_versioned_store, ScrubMode, ScrubResult};
pub use store::{
    remove_kept_safety_version, run_backup_cycle, simulate_backup_cycle,
    simulate_backup_cycle_with_sample_limit, BackupCycleResult, FolderBackupResult,
    FolderDescriptor, SimulationSummary,
};

/// Callers outside the core store implementation should not re-encode store layout rules.
pub fn store_root_path(destination_root: &std::path::Path) -> std::path::PathBuf {
    store::store_root(destination_root)
}

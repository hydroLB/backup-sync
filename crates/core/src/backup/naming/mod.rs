use chrono::Utc;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Purpose: Computes the backup directory for a given original path.
///
/// Inputs: the backup root and the original source path.
/// Outputs: the directory where backups for the file are stored.
/// Ties to: backup execution path preparation and hashing logic.
/// Side effects: None.
/// Why: keep backup location deterministic per source path.
pub fn backup_dir_for(root: &Path, original: &Path) -> PathBuf {
    let hashed = crate::fs::paths::hash_path(original);
    root.join("files").join(hashed)
}

/// Purpose: Builds the timestamped backup filename for a given source path.
///
/// Inputs: the original source path.
/// Outputs: a timestamped filename with the original name suffix.
/// Ties to: backup execution path preparation.
/// Side effects: None.
/// Why: allow multiple versions while preserving source name context.
pub fn backup_filename(original: &Path) -> String {
    let now = Utc::now();
    let stamp = now.format("%Y%m%d-%H%M%S").to_string();
    let name = match original.file_name() {
        Some(value) => value.to_string_lossy(),
        None => {
            warn!(
                "backup::naming::backup_filename missing file name for {:?}",
                original
            );
            "unknown".into()
        }
    };
    format!("{}__{}", stamp, name)
}

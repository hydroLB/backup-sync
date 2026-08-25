use chrono::Utc;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Keep backup location deterministic per source path.
pub fn backup_dir_for(root: &Path, original: &Path) -> PathBuf {
    let hashed = crate::fs::paths::hash_path(original);
    root.join("files").join(hashed)
}

/// Allow multiple versions while preserving source name context.
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

use std::path::{Path, PathBuf};

/// Purpose: Generates a stable hash for a filesystem path.
///
/// Inputs: the filesystem path to hash.
/// Outputs: a hex encoded hash string.
/// Ties to: backup naming and directory sharding.
/// Side effects: None.
/// Why: avoid long or unsafe path segments when building backup directories.
pub fn hash_path(path: &Path) -> String {
    let norm = path.to_string_lossy();
    let mut hasher = blake3::Hasher::new();
    hasher.update(norm.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Purpose: Joins a root path with a child path without normalization.
///
/// Inputs: the root and child paths.
/// Outputs: the joined `PathBuf`.
/// Ties to: path assembly for filesystem operations.
/// Side effects: None.
/// Why: keep path joining explicit and consistent across callers.
pub fn safe_join(root: &Path, child: &Path) -> PathBuf {
    let mut p = root.to_path_buf();
    p.push(child);
    p
}

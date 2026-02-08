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
    crate::hashing::sha256_hex(norm.as_bytes())
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

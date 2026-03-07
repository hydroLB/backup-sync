use std::path::{Path, PathBuf};

/// Summary: Generates a stable hash for a filesystem path.
///
/// Inputs: the filesystem path to hash.
///
/// Outputs: a hex encoded hash string.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: backup naming and directory sharding.
///
/// Why this exists: avoid long or unsafe path segments when building backup directories.
pub fn hash_path(path: &Path) -> String {
    let norm = path.to_string_lossy();
    crate::hashing::sha256_hex(norm.as_bytes())
}

/// Summary: Joins a root path with a child path without normalization.
///
/// Inputs: the root and child paths.
///
/// Outputs: the joined `PathBuf`.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: path assembly for filesystem operations.
///
/// Why this exists: keep path joining explicit and consistent across callers.
pub fn safe_join(root: &Path, child: &Path) -> PathBuf {
    let mut p = root.to_path_buf();
    p.push(child);
    p
}

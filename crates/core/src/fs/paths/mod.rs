use std::path::{Path, PathBuf};

/// Avoid long or unsafe path segments when building backup directories.
pub fn hash_path(path: &Path) -> String {
    let norm = path.to_string_lossy();
    crate::hashing::sha256_hex(norm.as_bytes())
}

/// Keep path joining explicit and consistent across callers.
pub fn safe_join(root: &Path, child: &Path) -> PathBuf {
    let mut p = root.to_path_buf();
    p.push(child);
    p
}

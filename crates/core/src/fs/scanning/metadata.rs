use std::path::PathBuf;

#[derive(Debug, Clone)]
/// Summary: Metadata snapshot for a scanned file.
///
/// Inputs: derived from filesystem metadata and config.
///
/// Outputs: a compact per file metadata record.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: backup planning and change detection.
///
/// Why this exists: provide consistent inputs for planning logic.
pub struct FileMeta {
    pub path: PathBuf,
    pub len: u64,
    pub mtime: i64,
    pub key: String,
    pub destination_root: PathBuf,
    pub max_copies: usize,
}

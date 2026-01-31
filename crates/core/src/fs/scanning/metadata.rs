use std::path::PathBuf;

#[derive(Debug, Clone)]
/// Purpose: Metadata snapshot for a scanned file.
///
/// Inputs: derived from filesystem metadata and config.
/// Outputs: a compact per file metadata record.
/// Ties to: backup planning and change detection.
/// Side effects: None.
/// Why: provide consistent inputs for planning logic.
pub struct FileMeta {
    pub path: PathBuf,
    pub len: u64,
    pub mtime: i64,
    pub key: String,
    pub destination_root: PathBuf,
    pub max_copies: usize,
}

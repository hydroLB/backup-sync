use std::path::PathBuf;

#[derive(Debug, Clone)]
/// Provide consistent inputs for planning logic.
pub struct FileMeta {
    pub path: PathBuf,
    pub len: u64,
    pub mtime: i64,
    pub key: String,
    pub destination_root: PathBuf,
    pub max_copies: usize,
}

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionIndex {
    pub schema_version: u32,
    pub source_path: String,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadFailurePhase {
    Walk,
    Metadata,
    Hash,
    BlobWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFailure {
    pub rel_path: String,
    pub phase: ReadFailurePhase,
    pub attempts: u32,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestEntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub kind: ManifestEntryKind,
    pub rel_path: String,
    pub len: u64,
    pub mtime_unix: i64,
    #[serde(default)]
    pub mtime_nanos: u32,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub schema_version: u32,
    pub source_path: String,
    pub created_at_unix: i64,
    #[serde(default)]
    pub read_failures: Vec<ReadFailure>,
    pub entries: BTreeMap<String, ManifestEntry>,
}

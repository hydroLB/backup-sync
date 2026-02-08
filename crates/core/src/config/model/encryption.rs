use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Summary: Configuration for at-rest encryption of blob contents.
///
/// Inputs: Loaded from the config file under `[encryption]`.
/// Outputs: Controls whether blobs are encrypted, where the key is loaded from, and the chunking policy.
/// Side effects: None.
/// Error handling: Validated by `config::validate` when enabled.
/// Ties to other methods: Used by versioned blob write, restore, and scrub routines.
/// Why this exists: Provide opt-in confidentiality for stored blob bytes without changing the manifest schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub key_path: Option<PathBuf>,
    #[serde(default)]
    pub key_id: Option<String>,
    #[serde(default = "crate::config::model::defaults::default_blob_encryption_chunk_bytes")]
    pub blob_chunk_bytes: usize,
}

impl Default for EncryptionConfig {
    /// Summary: Build the default encryption configuration.
    ///
    /// Inputs: None.
    /// Outputs: A disabled encryption config with safe defaults.
    /// Side effects: None.
    /// Error handling: None.
    /// Ties to other methods: Used by config defaults and serde `default` behavior.
    /// Why this exists: Keep encryption opt-in while making the config schema forward compatible.
    fn default() -> Self {
        Self {
            enabled: false,
            key_path: None,
            key_id: None,
            blob_chunk_bytes: crate::config::model::defaults::default_blob_encryption_chunk_bytes(),
        }
    }
}

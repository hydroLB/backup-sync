use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashingTuning {
    #[serde(default = "crate::config::model::defaults::default_hash_buffer_bytes")]
    pub buffer_bytes: usize,
    #[serde(default = "crate::config::model::defaults::default_hash_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for HashingTuning {
    /// Purpose: Builds a baseline hashing tuning profile for buffer sizing and timeouts.
    ///
    /// Inputs: the default functions in `config::model::defaults`.
    /// Outputs: a fully populated hashing tuning profile.
    /// Ties to: hashing in planning, execution, and verification.
    /// Side effects: None.
    /// Why: centralize hashing knobs so defaults stay aligned across the codebase.
    fn default() -> Self {
        Self {
            buffer_bytes: crate::config::model::defaults::default_hash_buffer_bytes(),
            timeout_seconds: crate::config::model::defaults::default_hash_timeout_seconds(),
        }
    }
}

use crate::state::models::StoredState;

/// Summary: Provides aggregate metrics derived from stored state.
///
/// Inputs: a snapshot of stored state.
///
/// Outputs: counts derived from the state.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: status reporting and diagnostics.
///
/// Why this exists: centralize derived metrics for reuse across the app.
#[derive(Debug, Clone)]
pub struct BackupIndex {
    file_count: usize,
    backup_count: usize,
}

impl BackupIndex {
    /// Summary: Builds an index from stored state values.
    ///
    /// Inputs: a reference to stored state.
    ///
    /// Outputs: a `BackupIndex` with aggregated counts.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: CLI and GUI status summaries.
    ///
    /// Why this exists: avoid recomputing counts in multiple call sites.
    pub fn from_state(state: &StoredState) -> Self {
        let file_count = state.files.len();
        let backup_count = state.files.values().map(|entry| entry.backups.len()).sum();
        Self {
            file_count,
            backup_count,
        }
    }

    /// Summary: Returns the number of tracked files.
    ///
    /// Inputs: none.
    ///
    /// Outputs: the tracked file count.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: status summaries.
    ///
    /// Why this exists: expose a simple count without recomputation.
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    /// Summary: Returns the number of stored backup copies.
    ///
    /// Inputs: none.
    ///
    /// Outputs: the backup copy count.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: status summaries.
    ///
    /// Why this exists: report how many backup files are tracked.
    pub fn backup_count(&self) -> usize {
        self.backup_count
    }
}

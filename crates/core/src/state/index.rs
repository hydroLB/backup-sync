use crate::state::models::StoredState;

/// Purpose: Provides aggregate metrics derived from stored state.
///
/// Inputs: a snapshot of stored state.
/// Outputs: counts derived from the state.
/// Ties to: status reporting and diagnostics.
/// Side effects: None.
/// Why: centralize derived metrics for reuse across the app.
#[derive(Debug, Clone)]
pub struct BackupIndex {
    file_count: usize,
    backup_count: usize,
}

impl BackupIndex {
    /// Purpose: Builds an index from stored state values.
    ///
    /// Inputs: a reference to stored state.
    /// Outputs: a `BackupIndex` with aggregated counts.
    /// Ties to: CLI and GUI status summaries.
    /// Side effects: None.
    /// Why: avoid recomputing counts in multiple call sites.
    pub fn from_state(state: &StoredState) -> Self {
        let file_count = state.files.len();
        let backup_count = state.files.values().map(|entry| entry.backups.len()).sum();
        Self {
            file_count,
            backup_count,
        }
    }

    /// Purpose: Returns the number of tracked files.
    ///
    /// Inputs: none.
    /// Outputs: the tracked file count.
    /// Ties to: status summaries.
    /// Side effects: None.
    /// Why: expose a simple count without recomputation.
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    /// Purpose: Returns the number of stored backup copies.
    ///
    /// Inputs: none.
    /// Outputs: the backup copy count.
    /// Ties to: status summaries.
    /// Side effects: None.
    /// Why: report how many backup files are tracked.
    pub fn backup_count(&self) -> usize {
        self.backup_count
    }
}

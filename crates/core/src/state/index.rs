use crate::state::models::StoredState;

/// Centralize derived metrics for reuse across the app.
#[derive(Debug, Clone)]
pub struct BackupIndex {
    file_count: usize,
    backup_count: usize,
}

impl BackupIndex {
    /// Avoid recomputing counts in multiple call sites.
    pub fn from_state(state: &StoredState) -> Self {
        let file_count = state.files.len();
        let backup_count = state.files.values().map(|entry| entry.backups.len()).sum();
        Self {
            file_count,
            backup_count,
        }
    }

    /// Expose a simple count without recomputation.
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    /// Report how many backup files are tracked.
    pub fn backup_count(&self) -> usize {
        self.backup_count
    }
}

use super::dirty::DirtySet;
use std::time::Duration;
use tokio::time::sleep;

/// Reduce churn by batching bursts of file events.
pub async fn debounce_and_take(dirty: &DirtySet, wait: Duration) -> Vec<std::path::PathBuf> {
    sleep(wait).await;
    super::watcher::take_dirty(dirty)
}

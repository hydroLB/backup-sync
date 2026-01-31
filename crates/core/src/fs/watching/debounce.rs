use super::dirty::DirtySet;
use std::time::Duration;
use tokio::time::sleep;

/// Purpose: Waits to coalesce rapid filesystem events before draining the dirty set.
///
/// Inputs: the dirty set and the debounce duration.
/// Outputs: a list of paths marked dirty after the wait.
/// Ties to: watcher debounce logic and dirty set consumption.
/// Side effects: Sleeps for the debounce duration.
/// Why: reduce churn by batching bursts of file events.
pub async fn debounce_and_take(dirty: &DirtySet, wait: Duration) -> Vec<std::path::PathBuf> {
    sleep(wait).await;
    super::watcher::take_dirty(dirty)
}

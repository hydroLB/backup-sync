use super::dirty::DirtySet;
use std::time::Duration;
use tokio::time::sleep;

/// Summary: Waits to coalesce rapid filesystem events before draining the dirty set.
///
/// Inputs: the dirty set and the debounce duration.
///
/// Outputs: a list of paths marked dirty after the wait.
///
/// Side effects: Sleeps for the debounce duration.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: watcher debounce logic and dirty set consumption.
///
/// Why this exists: reduce churn by batching bursts of file events.
pub async fn debounce_and_take(dirty: &DirtySet, wait: Duration) -> Vec<std::path::PathBuf> {
    sleep(wait).await;
    super::watcher::take_dirty(dirty)
}

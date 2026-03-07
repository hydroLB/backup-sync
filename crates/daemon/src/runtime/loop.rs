use super::signals::shutdown_signal;
use super::{cycle, ipc};
use anyhow::{Context, Result};
use backup_core::{
    fs::watching::{start_watcher, DirtySet},
    Config, IntervalScheduler, StateStore, StoredState,
};
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{error, info, span, Level};

/// Summary: Runs the daemon main loop with watchers, IPC, and scheduled tasks.
///
/// Inputs: the config, initial state, and state store.
///
/// Outputs: `Ok(())` when the loop exits cleanly.
///
/// Side effects: Starts watchers, spawns background tasks, and writes state on shutdown.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon startup and shutdown handling.
///
/// Why this exists: centralize daemon orchestration in one entry point.
pub(crate) async fn run(cfg: Config, mut state: StoredState, store: StateStore) -> Result<()> {
    state.safe_mode = cfg.safe_mode;
    let shared_state = Arc::new(Mutex::new(state));
    let dirty = DirtySet::default();
    let watcher_paths = cfg
        .watched
        .iter()
        .map(|w| w.path.clone())
        .collect::<Vec<_>>();
    if watcher_paths.is_empty() {
        tracing::warn!("daemon::runtime::run no watched paths configured; daemon will idle");
    }
    let _watcher = start_watcher(watcher_paths, dirty.clone())
        .context("daemon::runtime::run failed to start file watcher")?;
    let scheduler = IntervalScheduler::new_secs(cfg.interval_seconds);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let ipc_handle = ipc::spawn_server(
        shared_state.clone(),
        cfg.destinations.clone(),
        Duration::from_secs(cfg.runtime.ipc_timeout_seconds),
        cfg.runtime.ipc_request_max_bytes,
        cfg.runtime.ipc_read_chunk_bytes,
        cfg.execution.recent_activity_cap,
        shutdown_rx.clone(),
    )
    .await
    .context("daemon::runtime::run failed to start IPC server")?;

    if let Err(e) = cycle::run_cycle(&cfg, &dirty, &store, &shared_state).await {
        error!("daemon::runtime::run initial backup cycle failed: {e:?}");
    }

    let schedule_task = {
        let cfg = cfg.clone();
        let store = store.clone();
        let dirty = dirty.clone();
        let shared_state = shared_state.clone();
        let mut scheduler_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            scheduler
                .run_until_shutdown(&mut scheduler_shutdown, || {
                    let cfg = cfg.clone();
                    let store = store.clone();
                    let dirty = dirty.clone();
                    let shared_state = shared_state.clone();
                    async move {
                        let span = span!(Level::INFO, "backup_cycle");
                        let _g = span.enter();
                        if let Err(e) = cycle::run_cycle(&cfg, &dirty, &store, &shared_state).await
                        {
                            error!("daemon::runtime::run backup cycle failed: {e:?}");
                        }
                    }
                })
                .await;
        })
    };

    let verify_task = {
        let cfg = cfg.clone();
        let shared_state = shared_state.clone();
        let store = store.clone();
        let verify_interval = cfg.runtime.verify_interval_seconds;
        let hashing = cfg.hashing.clone();
        let mut verify_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(verify_interval));
            loop {
                tokio::select! {
                    changed = verify_shutdown.changed() => {
                        match changed {
                            Ok(()) | Err(_) => break,
                        }
                    }
                    _ = interval.tick() => {
                        if let Err(e) = cycle::run_verify_cycle(&cfg, &store, &shared_state, &hashing).await
                        {
                            error!("daemon::runtime::run periodic verify failed: {e:?}");
                        }
                    }
                }
            }
        })
    };

    shutdown_signal().await;
    info!("shutdown signal received");
    if let Err(error) = shutdown_tx.send(true) {
        error!("daemon::runtime::run failed to broadcast shutdown signal: {error}");
    }
    let shutdown_join_timeout =
        Duration::from_secs(cfg.runtime.daemon_shutdown_join_timeout_seconds.max(1));
    join_task_with_timeout("ipc", ipc_handle, shutdown_join_timeout).await;
    join_task_with_timeout("scheduler", schedule_task, shutdown_join_timeout).await;
    join_task_with_timeout("verify", verify_task, shutdown_join_timeout).await;
    let state = shared_state.lock().await.clone();
    if let Err(e) = store.persist(&state) {
        error!("daemon::runtime::run failed to persist state during shutdown: {e:?}");
    }
    Ok(())
}

/// Summary: Awaits a spawned daemon task with a timeout and abort fallback.
///
/// Inputs: task label, join handle, and timeout duration.
///
/// Outputs: `()` after join completion or forced abort.
///
/// Side effects: May abort the task when graceful completion exceeds the timeout.
///
/// Error handling: Emits structured errors for join failures and forced aborts.
///
/// Ties to other methods: used by daemon shutdown coordination in `run`.
///
/// Why this exists: avoid hanging shutdown while preferring graceful task completion over immediate aborts.
async fn join_task_with_timeout(label: &str, mut handle: JoinHandle<()>, timeout: Duration) {
    match tokio::time::timeout(timeout, &mut handle).await {
        Ok(Ok(())) => {}
        Ok(Err(join_error)) => {
            error!("daemon::runtime::run {label} task join failed during shutdown: {join_error}");
        }
        Err(_) => {
            error!(
                "daemon::runtime::run {label} task did not exit within {:?}; aborting",
                timeout
            );
            handle.abort();
            if let Err(join_error) = handle.await {
                error!("daemon::runtime::run {label} task join failed after abort: {join_error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::join_task_with_timeout;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::time::Duration;

    struct DropFlag {
        dropped: Arc<AtomicBool>,
    }

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    /// Summary: Ensures graceful join returns immediately when task exits before timeout.
    ///
    /// Inputs: a completed task and a bounded timeout.
    ///
    /// Outputs: successful completion without panic.
    ///
    /// Side effects: Spawns and joins a tokio task.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `join_task_with_timeout`.
    ///
    /// Why this exists: verify shutdown coordination does not regress when tasks finish cleanly.
    async fn join_task_with_timeout_accepts_completed_task() {
        let handle = tokio::spawn(async {});
        join_task_with_timeout("test-complete", handle, Duration::from_millis(20)).await;
    }

    #[tokio::test]
    /// Summary: Ensures shutdown fallback aborts tasks that exceed graceful join timeout.
    ///
    /// Inputs: a never-completing task and a short join timeout.
    ///
    /// Outputs: assertion that task cleanup ran via drop after abort.
    ///
    /// Side effects: Spawns and aborts a tokio task.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `join_task_with_timeout`.
    ///
    /// Why this exists: prove daemon shutdown enforces bounded exit even when tasks hang.
    async fn join_task_with_timeout_aborts_hung_task() {
        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_for_task = dropped.clone();
        let handle = tokio::spawn(async move {
            let _guard = DropFlag {
                dropped: dropped_for_task,
            };
            std::future::pending::<()>().await;
        });
        join_task_with_timeout("test-hung", handle, Duration::from_millis(10)).await;
        assert!(
            dropped.load(Ordering::SeqCst),
            "hung task should be aborted and dropped after timeout"
        );
    }
}

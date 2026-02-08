use super::signals::shutdown_signal;
use super::{cycle, ipc};
use anyhow::{Context, Result};
use backup_core::{
    fs::watching::{start_watcher, DirtySet},
    scheduling::interval::runner::IntervalScheduler,
    Config, StateStore, StoredState,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing::{error, info, span, Level};

const FIXED_AUTOMATIC_INTERVAL_SECONDS: u64 = 30 * 60;

/// Purpose: Runs the daemon main loop with watchers, IPC, and scheduled tasks.
///
/// Inputs: the config, initial state, and state store.
/// Outputs: `Ok(())` when the loop exits cleanly.
/// Ties to: daemon startup and shutdown handling.
/// Side effects: Starts watchers, spawns background tasks, and writes state on shutdown.
/// Why: centralize daemon orchestration in one entry point.
pub async fn run(cfg: Config, mut state: StoredState, store: StateStore) -> Result<()> {
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
    let scheduler = IntervalScheduler::new_secs(FIXED_AUTOMATIC_INTERVAL_SECONDS);
    let ipc_handle = ipc::spawn_server(
        shared_state.clone(),
        cfg.destinations.clone(),
        Duration::from_secs(cfg.runtime.ipc_timeout_seconds),
        cfg.execution.recent_activity_cap,
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
        tokio::spawn(async move {
            scheduler
                .run(|| {
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
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(verify_interval));
            loop {
                interval.tick().await;
                if let Err(e) = cycle::run_verify_cycle(&cfg, &store, &shared_state, &hashing).await
                {
                    error!("daemon::runtime::run periodic verify failed: {e:?}");
                }
            }
        })
    };

    shutdown_signal().await;
    info!("shutdown signal received");
    let state = shared_state.lock().await.clone();
    if let Err(e) = store.persist(&state) {
        error!("daemon::runtime::run failed to persist state during shutdown: {e:?}");
    }
    ipc_handle.abort();
    schedule_task.abort();
    verify_task.abort();
    let _ = ipc_handle.await;
    let _ = schedule_task.await;
    let _ = verify_task.await;
    Ok(())
}

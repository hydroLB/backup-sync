use super::dirty::DirtySet;
use crate::config::model::RuntimeTuning;
use crate::logging::redact_path;
use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::PathBuf, time::Duration};
use tracing::warn;

/// Purpose: Starts a filesystem watcher and populates a shared dirty set.
///
/// Inputs: a list of paths to watch and a dirty set to update.
/// Outputs: a configured watcher or an error with context.
/// Ties to: daemon and GUI background monitoring.
/// Side effects: Registers filesystem watches and mutates the dirty set on events.
/// Why: keep change detection reactive without rescanning every cycle.
pub fn start_watcher(paths: Vec<PathBuf>, dirty: DirtySet) -> Result<RecommendedWatcher> {
    let mut watcher =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(ev) => {
                if matches!(
                    ev.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                ) {
                    for p in ev.paths {
                        dirty.0.lock().insert(p);
                    }
                }
            }
            Err(e) => {
                warn!("fs::watching::start_watcher watcher event error: {:?}", e);
            }
        })
        .context("fs::watching::start_watcher failed to create file watcher")?;
    for p in paths {
        if !p.exists() {
            warn!(
                "fs::watching::start_watcher watched path does not exist, skipping: {}",
                redact_path(&p)
            );
            continue;
        }
        watcher
            .watch(&p, RecursiveMode::Recursive)
            .with_context(|| format!("fs::watching::start_watcher failed to watch path {:?}", p))?;
    }
    Ok(watcher)
}

/// Purpose: Drains and returns the current dirty path set.
///
/// Inputs: a shared dirty set.
/// Outputs: a vector of paths that were marked dirty.
/// Ties to: debounce and scan filtering.
/// Side effects: Mutates the dirty set by draining stored paths.
/// Why: clear dirty state after it has been consumed.
pub fn take_dirty(dirty: &DirtySet) -> Vec<PathBuf> {
    let mut guard = dirty.0.lock();
    guard.drain().collect()
}

/// Purpose: Returns the debounce duration for watchers.
///
/// Inputs: the runtime tuning configuration.
/// Outputs: the debounce duration.
/// Ties to: debounce logic for event coalescing.
/// Side effects: None.
/// Why: centralize debounce timing configuration.
pub fn debounce_duration(runtime: &RuntimeTuning) -> Duration {
    Duration::from_secs(runtime.watcher_debounce_seconds)
}

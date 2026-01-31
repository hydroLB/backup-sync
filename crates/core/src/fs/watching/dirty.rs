use parking_lot::Mutex;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

/// Purpose: Shared set of dirty paths captured by watchers.
///
/// Inputs: file paths inserted by watcher callbacks.
/// Outputs: a thread safe set of dirty paths.
/// Ties to: watcher event batching and scan filtering.
/// Side effects: None.
/// Why: track changed paths between scan cycles.
#[derive(Clone, Default)]
pub struct DirtySet(pub Arc<Mutex<HashSet<PathBuf>>>);

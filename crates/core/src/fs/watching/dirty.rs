use parking_lot::Mutex;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

/// Summary: Shared set of dirty paths captured by watchers.
///
/// Inputs: file paths inserted by watcher callbacks.
///
/// Outputs: a thread safe set of dirty paths.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: watcher event batching and scan filtering.
///
/// Why this exists: track changed paths between scan cycles.
#[derive(Clone, Default)]
pub struct DirtySet(pub Arc<Mutex<HashSet<PathBuf>>>);

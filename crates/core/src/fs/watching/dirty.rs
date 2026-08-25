use parking_lot::Mutex;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

/// Track changed paths between scan cycles.
#[derive(Clone, Default)]
pub struct DirtySet(pub Arc<Mutex<HashSet<PathBuf>>>);

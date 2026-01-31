pub mod defaults;
pub mod tuning;

mod types;

pub use tuning::{ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning};
pub use types::{Config, Destination, WatchedKind, WatchedPath};

pub(crate) use defaults::{
    default_destination_id, default_ignore_patterns, default_max_bytes_per_second,
    default_max_parallel_copies, default_min_free_space_bytes, default_skip_hidden,
};

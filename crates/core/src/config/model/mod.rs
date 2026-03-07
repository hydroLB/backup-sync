pub mod compression;
pub mod defaults;
pub mod encryption;
pub mod tuning;

mod types;

pub use compression::CompressionConfig;
pub use encryption::EncryptionConfig;
pub use tuning::{ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning};
pub use types::{Config, Destination, WatchedKind, WatchedPath};

pub(crate) use defaults::{
    default_destination_id, default_ignore_patterns, default_interval_seconds,
    default_max_backups_per_file, default_max_bytes_per_second, default_max_parallel_copies,
    default_min_free_space_bytes, default_skip_hidden,
};

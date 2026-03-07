//! Core crate public API surface.
//!
//! Keep external contracts explicit via this file:
//! - only modules intentionally consumed by other crates are `pub mod`.
//! - shared entrypoints and model types are re-exported below.

pub mod backup;
pub mod boundary_error;
pub mod config;
pub mod encryption;
pub mod fs;
pub mod io;
pub mod logging;
pub mod metrics;
pub mod platform;
pub mod service;
pub mod state;

mod hashing;
mod scheduling;

pub use backup::execution::{BackupExecutor, BackupResult};
pub use backup::planning::{BackupPlan, PlannedItem};
pub use boundary_error::{classify_anyhow, CanonicalErrorCode, ClassifiedBoundaryError};
pub use config::model::{Config, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning};
pub use config::{
    load_config, load_from_path, load_validated_config, load_validated_from_path, save_config,
    validate, validate_with_limits, ValidationLimits,
};
pub use fs::watching::dirty::DirtySet;
pub use hashing::{
    sha256_file_hex, sha256_file_hex_with_tuning, sha256_hex, CONTENT_HASH_ALGORITHM,
};
pub use scheduling::interval::runner::IntervalScheduler;
pub use state::models::{ActivityItem, FileState, SafetyWarning, StoredState};
pub use state::store::StateStore;
pub use state::verify::verify_backups;

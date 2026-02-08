pub mod backup;
pub mod config;
pub mod encryption;
pub mod fs;
pub mod hashing;
pub mod logging;
pub mod platform;
pub mod scheduling;
pub mod state;

#[cfg(feature = "legacy-engine")]
pub use backup::execution::{BackupExecutor, BackupResult};
#[cfg(feature = "legacy-engine")]
pub use backup::planning::{BackupPlan, PlannedItem};
pub use config::model::{Config, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning};
pub use config::{
    load_config, load_from_path, save_config, validate, validate_with_limits, ValidationLimits,
};
pub use fs::watching::dirty::DirtySet;
pub use scheduling::interval::runner::IntervalScheduler;
pub use state::models::{ActivityItem, FileState, SafetyWarning, StoredState};
pub use state::store::StateStore;
#[cfg(feature = "legacy-engine")]
pub use state::verify::verify_backups;

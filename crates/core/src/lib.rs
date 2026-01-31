pub mod backup;
pub mod config;
pub mod fs;
pub mod logging;
pub mod platform;
pub mod scheduling;
pub mod state;

pub use backup::execution::{BackupExecutor, BackupResult};
pub use backup::planning::{BackupPlan, PlannedItem};
pub use config::model::{Config, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning};
pub use config::{
    load_config, load_from_path, save_config, validate, validate_with_limits, ValidationLimits,
};
pub use fs::watching::dirty::DirtySet;
pub use scheduling::interval::runner::IntervalScheduler;
pub use state::models::{ActivityItem, FileState, StoredState};
pub use state::store::StateStore;
pub use state::verify::verify_backups;

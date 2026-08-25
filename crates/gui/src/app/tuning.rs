use backup_core::config::model::RuntimeTuning;
use backup_core::load_validated_config;
use tracing::warn;

/// Keep GUI timing knobs configurable without hardcoded values.
pub(crate) fn resolve_runtime_tuning() -> RuntimeTuning {
    match load_validated_config() {
        Ok(cfg) => cfg.runtime,
        Err(e) => {
            warn!(
                "gui::app::tuning::resolve_runtime_tuning failed to load config; using defaults: {}",
                e
            );
            RuntimeTuning::default()
        }
    }
}

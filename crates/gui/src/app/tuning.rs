use backup_core::config::model::RuntimeTuning;
use backup_core::load_config;
use tracing::warn;

/// Purpose: Loads runtime tuning for GUI lifecycle behavior.
///
/// Inputs: none.
/// Outputs: runtime tuning from config or defaults.
/// Ties to: tray refresh intervals and GUI startup timing.
/// Side effects: Reads config from disk and logs warnings on fallback.
/// Why: keep GUI timing knobs configurable without hardcoded values.
pub(crate) fn resolve_runtime_tuning() -> RuntimeTuning {
    match load_config() {
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

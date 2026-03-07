use backup_core::config::model::RuntimeTuning;
use backup_core::load_validated_config;
use tracing::warn;

/// Summary: Loads runtime tuning for GUI lifecycle behavior.
///
/// Inputs: none.
///
/// Outputs: runtime tuning from config or defaults.
///
/// Side effects: Reads config from disk and logs warnings on fallback.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray refresh intervals and GUI startup timing.
///
/// Why this exists: keep GUI timing knobs configurable without hardcoded values.
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

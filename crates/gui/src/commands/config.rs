use crate::api::{config_api, status_api};
use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use tauri::async_runtime;
use tracing::warn;

#[tauri::command]
/// Summary: Loads the current configuration for the GUI.
///
/// Inputs: none.
///
/// Outputs: the current config or an error envelope.
///
/// Side effects: Reads the config file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI settings screens.
///
/// Why this exists: allow the frontend to display and edit configuration.
pub fn load_config_cmd() -> Result<backup_core::Config, ErrorEnvelope> {
    config_api::get_config().map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "CONFIG_LOAD",
            "config::load_config_cmd failed to load config",
            &e,
        )
    })
}

#[tauri::command]
/// Summary: Saves a configuration update from the GUI.
///
/// Inputs: the new config, optional correlation id, and auth state.
///
/// Outputs: `Ok(())` when the config is persisted.
///
/// Side effects: Writes the config file to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI settings save actions.
///
/// Why this exists: allow the frontend to update configuration with auth checks.
pub fn save_config_cmd(
    cfg: backup_core::Config,
    _correlation_id: Option<String>,
) -> Result<(), ErrorEnvelope> {
    config_api::save_config(&cfg).map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "CONFIG_SAVE",
            "config::save_config_cmd failed to save config",
            &e,
        )
    })?;
    if let Err(error) = crate::commands::service::restart_daemon_cmd(None) {
        warn!(
            action = "restart_daemon_after_save_failed",
            error = %error.message,
            "config::save_config_cmd failed to refresh daemon after config save"
        );
    }
    Ok(())
}

#[tauri::command]
/// Summary: Toggles safe mode in the configuration and persists the change.
///
/// Inputs: an optional desired value, correlation id, and auth state.
///
/// Outputs: the new safe mode state or an error envelope.
///
/// Side effects: Reads and writes the config file to persist safe mode changes.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI safe mode switches.
///
/// Why this exists: provide a simple toggle for enabling or disabling safe mode.
pub async fn toggle_safe_mode_cmd(
    desired: Option<bool>,
    correlation_id: Option<String>,
) -> Result<bool, ErrorEnvelope> {
    let cid = correlation::cid("toggle-safe-mode", correlation_id);
    let mut cfg = config_api::get_config().map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "CONFIG_LOAD",
            "config::toggle_safe_mode_cmd failed to load config",
            &e,
        )
    })?;
    let next = desired.unwrap_or(!cfg.safe_mode);
    cfg.safe_mode = next;
    config_api::save_config(&cfg).map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "CONFIG_SAVE",
            "config::toggle_safe_mode_cmd failed to save config",
            &e,
        )
    })?;
    async_runtime::spawn(async move {
        if let Err(e) = status_api::set_safe_mode_with_correlation(next, Some(cid.as_str())).await {
            warn!(
                cid = %cid,
                action = "set_safe_mode_ipc_failed",
                error = %e,
                "config::toggle_safe_mode_cmd failed to update daemon safe mode"
            );
        }
    });
    Ok(next)
}

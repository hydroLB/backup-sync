use crate::api::{config_api, status_api};
use crate::commands::auth;
use crate::commands::error::ErrorEnvelope;
use crate::commands::security;
use tauri::State;
use tracing::warn;

#[tauri::command]
/// Purpose: Loads the current configuration for the GUI.
///
/// Inputs: none.
/// Outputs: the current config or an error envelope.
/// Ties to: GUI settings screens.
/// Side effects: Reads the config file from disk.
/// Why: allow the frontend to display and edit configuration.
pub fn load_config_cmd() -> Result<backup_core::Config, ErrorEnvelope> {
    config_api::get_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!("config::load_config_cmd failed to load config: {}", e),
        )
    })
}

#[tauri::command]
/// Purpose: Saves a configuration update from the GUI.
///
/// Inputs: the new config, optional correlation id, and auth state.
/// Outputs: `Ok(())` when the config is persisted.
/// Ties to: GUI settings save actions.
/// Side effects: Writes the config file to disk.
/// Why: allow the frontend to update configuration with auth checks.
pub fn save_config_cmd(
    cfg: backup_core::Config,
    correlation_id: Option<String>,
    auth_state: State<auth::SessionAuth>,
) -> Result<(), ErrorEnvelope> {
    security::ensure_unlocked(&auth_state, correlation_id.clone())?;
    config_api::save_config(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_SAVE",
            format!("config::save_config_cmd failed to save config: {}", e),
        )
    })
}

#[tauri::command]
/// Purpose: Toggles safe mode in the configuration and persists the change.
///
/// Inputs: an optional desired value, correlation id, and auth state.
/// Outputs: the new safe mode state or an error envelope.
/// Ties to: GUI safe mode switches.
/// Side effects: Reads and writes the config file to persist safe mode changes.
/// Why: provide a simple toggle for enabling or disabling safe mode.
pub async fn toggle_safe_mode_cmd(
    desired: Option<bool>,
    correlation_id: Option<String>,
    auth_state: State<'_, auth::SessionAuth>,
) -> Result<bool, ErrorEnvelope> {
    security::ensure_unlocked(&auth_state, correlation_id.clone())?;
    let mut cfg = config_api::get_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!("config::toggle_safe_mode_cmd failed to load config: {}", e),
        )
    })?;
    let next = desired.unwrap_or(!cfg.safe_mode);
    cfg.safe_mode = next;
    config_api::save_config(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_SAVE",
            format!("config::toggle_safe_mode_cmd failed to save config: {}", e),
        )
    })?;
    if let Err(e) = status_api::set_safe_mode(next).await {
        warn!("config::toggle_safe_mode_cmd failed to update daemon safe mode: {e}");
    }
    Ok(next)
}

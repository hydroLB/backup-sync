use crate::api::{config_api, status_api};
use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use serde::Serialize;
use tracing::warn;

#[derive(Serialize)]
pub struct ConfigSaveResult {
    daemon_restarted: bool,
    daemon_restart_warning: Option<String>,
}

#[derive(Serialize)]
pub struct SafeModeUpdateResult {
    safe_mode: bool,
    applied_live: bool,
    warning: Option<String>,
}

fn config_save_result_from_restart_result(
    restart_result: Result<String, ErrorEnvelope>,
) -> ConfigSaveResult {
    match restart_result {
        Ok(_) => ConfigSaveResult {
            daemon_restarted: true,
            daemon_restart_warning: None,
        },
        Err(error) => {
            warn!(
                action = "restart_daemon_after_save_failed",
                error = %error.message,
                "config::save_config_cmd failed to refresh daemon after config save"
            );
            ConfigSaveResult {
                daemon_restarted: false,
                daemon_restart_warning: Some(format!(
                    "Configuration saved, but the daemon could not be restarted automatically. Runtime changes may not apply until restart: {}",
                    error.message
                )),
            }
        }
    }
}

#[tauri::command]
/// Allow the frontend to display and edit configuration.
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
/// Allow the frontend to update configuration with auth checks.
pub async fn save_config_cmd(
    cfg: backup_core::Config,
    _correlation_id: Option<String>,
) -> Result<ConfigSaveResult, ErrorEnvelope> {
    config_api::save_config(&cfg).map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "CONFIG_SAVE",
            "config::save_config_cmd failed to save config",
            &e,
        )
    })?;
    let restart_result =
        tokio::task::spawn_blocking(|| crate::commands::service::restart_daemon_cmd(None))
            .await
            .map_err(|error| {
                ErrorEnvelope::new(
                    "BLOCKING_TASK_FAILED",
                    format!("config::save_config_cmd daemon restart task failed: {error}"),
                )
            })?;
    Ok(config_save_result_from_restart_result(restart_result))
}

#[tauri::command]
/// Provide a simple toggle for enabling or disabling safe mode.
pub async fn toggle_safe_mode_cmd(
    desired: Option<bool>,
    correlation_id: Option<String>,
) -> Result<SafeModeUpdateResult, ErrorEnvelope> {
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
    match status_api::set_safe_mode_with_correlation(next, Some(cid.as_str())).await {
        Ok(()) => Ok(SafeModeUpdateResult {
            safe_mode: next,
            applied_live: true,
            warning: None,
        }),
        Err(error) => {
            warn!(
                cid = %cid,
                action = "set_safe_mode_saved_for_next_start",
                error = %error,
                "safe mode was saved but the running daemon could not acknowledge it"
            );
            Ok(SafeModeUpdateResult {
                safe_mode: next,
                applied_live: false,
                warning: Some(
                    "Preference saved for the next daemon start; the background service is not currently responding."
                        .to_string(),
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{config_save_result_from_restart_result, ConfigSaveResult};
    use crate::commands::error::ErrorEnvelope;

    #[test]
    fn config_save_result_serializes_expected_wire_keys() {
        let result = ConfigSaveResult {
            daemon_restarted: false,
            daemon_restart_warning: Some("restart me".to_string()),
        };

        let value = serde_json::to_value(result).expect("config save result should serialize");
        assert_eq!(value["daemon_restarted"], serde_json::Value::Bool(false));
        assert_eq!(
            value["daemon_restart_warning"],
            serde_json::Value::String("restart me".to_string())
        );
    }

    #[test]
    fn config_save_result_maps_restart_success_to_clean_payload() {
        let result = config_save_result_from_restart_result(Ok("daemon restarted".to_string()));
        let value = serde_json::to_value(result).expect("config save result should serialize");
        assert_eq!(value["daemon_restarted"], serde_json::Value::Bool(true));
        assert_eq!(value["daemon_restart_warning"], serde_json::Value::Null);
    }

    #[test]
    fn config_save_result_maps_restart_failure_to_warning_payload() {
        let result = config_save_result_from_restart_result(Err(ErrorEnvelope::new(
            "SERVICE_RESTART",
            "restart failed",
        )));
        let value = serde_json::to_value(result).expect("config save result should serialize");
        assert_eq!(value["daemon_restarted"], serde_json::Value::Bool(false));
        assert!(value["daemon_restart_warning"]
            .as_str()
            .expect("warning string")
            .contains("restart failed"));
    }
}

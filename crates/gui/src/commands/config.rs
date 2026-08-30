use crate::api::{config_api, status_api};
use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::warn;

const CONFIG_RESTART_DEBOUNCE: Duration = Duration::from_millis(400);
static CONFIG_RESTART_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
pub struct ConfigSaveResult {
    daemon_restarted: bool,
    daemon_restart_scheduled: bool,
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
            daemon_restart_scheduled: false,
            daemon_restart_warning: None,
        },
        Err(error) => {
            warn!(
                action = "restart_daemon_after_source_removal_failed",
                error = %error.message,
                "config::remove_protected_path_cmd removed source history but could not refresh the daemon"
            );
            ConfigSaveResult {
                daemon_restarted: false,
                daemon_restart_scheduled: false,
                daemon_restart_warning: Some(format!(
                    "Protection and its saved versions were removed, but the background service could not restart automatically. Restart Backup Sync before changing more settings: {}",
                    error.message
                )),
            }
        }
    }
}

/// Coalesce rapid inline edits and refresh the daemon after the UI has already acknowledged the
/// durable config write. Restarting synchronously made every +/- click wait for process teardown,
/// launch, and IPC socket rebinding even though the config itself was saved in milliseconds.
fn schedule_daemon_restart() {
    let generation = CONFIG_RESTART_GENERATION
        .fetch_add(1, Ordering::SeqCst)
        .saturating_add(1);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(CONFIG_RESTART_DEBOUNCE).await;
        if CONFIG_RESTART_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        match tokio::task::spawn_blocking(|| crate::commands::service::restart_daemon_cmd(None))
            .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => warn!(
                action = "restart_daemon_after_save_failed",
                error = %error.message,
                "config::schedule_daemon_restart failed to refresh daemon after config save"
            ),
            Err(error) => warn!(
                action = "restart_daemon_after_save_task_failed",
                error = %error,
                "config::schedule_daemon_restart task failed"
            ),
        }
    });
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
    schedule_daemon_restart();
    Ok(ConfigSaveResult {
        daemon_restarted: false,
        daemon_restart_scheduled: true,
        daemon_restart_warning: None,
    })
}

#[tauri::command]
/// Remove one protected source and its committed history from every configured destination.
pub async fn remove_protected_path_cmd(
    cfg: backup_core::Config,
    source_path: String,
    kind: backup_core::config::model::WatchedKind,
    correlation_id: Option<String>,
) -> Result<ConfigSaveResult, ErrorEnvelope> {
    let source_path = PathBuf::from(source_path);
    let removed_source_path = source_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let previous = config_api::get_config().map_err(|error| {
            ErrorEnvelope::from_anyhow_with_code(
                "CONFIG_LOAD",
                "config::remove_protected_path_cmd failed to load current config",
                &error,
            )
        })?;
        let same_kind = |watched: &backup_core::config::model::WatchedPath| {
            matches!(
                (&watched.kind, &kind),
                (
                    backup_core::config::model::WatchedKind::File,
                    backup_core::config::model::WatchedKind::File
                ) | (
                    backup_core::config::model::WatchedKind::Directory,
                    backup_core::config::model::WatchedKind::Directory
                )
            )
        };
        if !previous
            .watched
            .iter()
            .any(|watched| watched.path == source_path && same_kind(watched))
        {
            return Err(ErrorEnvelope::new(
                "NOT_FOUND",
                "The protected path no longer exists in the current configuration.",
            ));
        }
        if cfg
            .watched
            .iter()
            .any(|watched| watched.path == source_path)
        {
            return Err(ErrorEnvelope::new(
                "CONFLICT",
                "Every destination entry for this protected path must be removed together.",
            ));
        }

        let mut expected = previous.clone();
        expected
            .watched
            .retain(|watched| watched.path != source_path || !same_kind(watched));
        let expected_value = serde_json::to_value(&expected).map_err(|error| {
            ErrorEnvelope::new(
                "INTERNAL",
                format!("Could not verify the expected protection change: {error}"),
            )
        })?;
        let requested_value = serde_json::to_value(&cfg).map_err(|error| {
            ErrorEnvelope::new(
                "INVALID_INPUT",
                format!("Could not verify the requested protection change: {error}"),
            )
        })?;
        if requested_value != expected_value {
            return Err(ErrorEnvelope::new(
                "CONFLICT",
                "The configuration changed while removal was being prepared. Reload and try again.",
            ));
        }
        backup_core::validate(&cfg).map_err(|error| {
            ErrorEnvelope::from_anyhow_with_code(
                "CONFIG_INVALID",
                "config::remove_protected_path_cmd rejected the resulting config",
                &error,
            )
        })?;

        let restart_result = backup_core::backup::versioned::remove_source_history_with_commit(
            &previous,
            &source_path,
            || {
                config_api::save_config(&cfg)?;
                Ok(crate::commands::service::restart_daemon_cmd(None))
            },
        )
        .map_err(|error| {
            ErrorEnvelope::from_anyhow_with_code(
                "CONFIG_SAVE",
                "config::remove_protected_path_cmd could not delete saved versions",
                &error,
            )
        })?;

        Ok(config_save_result_from_restart_result(restart_result))
    })
    .await
    .map_err(|error| {
        ErrorEnvelope::new(
            "BLOCKING_TASK_FAILED",
            format!("Protected path removal task failed: {error}"),
        )
    })??;

    // A shrink warning can race with an intentional removal. Only clear the warning when it
    // belongs to the source that was just removed; warnings for other protected sources remain.
    match status_api::fetch_status_with_correlation(correlation_id.as_deref()).await {
        Ok(status)
            if status
                .last_safety_warning
                .as_ref()
                .and_then(|warning| warning.watched_path.as_deref())
                .map(PathBuf::from)
                .as_ref()
                == Some(&removed_source_path) =>
        {
            if let Err(error) =
                status_api::clear_safety_warning_with_correlation(correlation_id.as_deref()).await
            {
                warn!(
                    action = "clear_removed_source_safety_warning_failed",
                    error = %error,
                    "source removal succeeded but its stale safety warning could not be cleared"
                );
            }
        }
        Ok(_) => {}
        Err(error) => warn!(
            action = "inspect_removed_source_safety_warning_failed",
            error = %error,
            "source removal succeeded but the daemon status was unavailable"
        ),
    }

    Ok(result)
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
            daemon_restart_scheduled: true,
            daemon_restart_warning: Some("restart me".to_string()),
        };

        let value = serde_json::to_value(result).expect("config save result should serialize");
        assert_eq!(value["daemon_restarted"], serde_json::Value::Bool(false));
        assert_eq!(
            value["daemon_restart_scheduled"],
            serde_json::Value::Bool(true)
        );
        assert_eq!(
            value["daemon_restart_warning"],
            serde_json::Value::String("restart me".to_string())
        );
    }

    #[test]
    fn source_removal_restart_failure_is_reported_without_claiming_removal_failed() {
        let result = config_save_result_from_restart_result(Err(ErrorEnvelope::new(
            "RESTART_FAILED",
            "daemon unavailable",
        )));
        let value = serde_json::to_value(result).expect("config save result should serialize");
        let warning = value["daemon_restart_warning"]
            .as_str()
            .expect("restart warning should be present");
        assert!(warning.contains("saved versions were removed"));
        assert!(warning.contains("daemon unavailable"));
    }
}

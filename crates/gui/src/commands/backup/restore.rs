use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use backup_core::backup::versioned::restore::{
    list_versions, restore_version, RestoreMode, RestoreRequest,
};
use backup_core::{load_config, validate};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct VersionInfoDto {
    pub id: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Serialize)]
pub struct FolderVersionsDto {
    pub source_path: String,
    pub versions: Vec<VersionInfoDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreModeDto {
    InPlace,
    ToDirectory,
}

#[derive(Debug, Deserialize)]
pub struct RestoreArgs {
    pub source_path: String,
    pub version_id: String,
    pub mode: RestoreModeDto,
    pub target_dir: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RestoreResultDto {
    pub files_written: usize,
    pub files_removed: usize,
    pub dirs_created: usize,
}

#[tauri::command]
pub async fn list_versions_cmd(
    correlation_id: Option<String>,
) -> Result<Vec<FolderVersionsDto>, ErrorEnvelope> {
    let cid = correlation::cid("restore_list", correlation_id);
    let cfg = load_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] list_versions_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;
    validate(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "[cid={}] list_versions_cmd config validation failed: {}",
                cid, e
            ),
        )
    })?;

    let listed = list_versions(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "LIST_FAILED",
            format!("[cid={}] list_versions_cmd failed: {}", cid, e),
        )
    })?;
    Ok(listed
        .into_iter()
        .map(|(path, versions)| FolderVersionsDto {
            source_path: path.to_string_lossy().into_owned(),
            versions: versions
                .into_iter()
                .map(|v| VersionInfoDto {
                    id: v.id,
                    created_at_unix: v.created_at_unix,
                })
                .collect(),
        })
        .collect())
}

#[tauri::command]
pub async fn restore_version_cmd(
    args: RestoreArgs,
    correlation_id: Option<String>,
) -> Result<RestoreResultDto, ErrorEnvelope> {
    let cid = correlation::cid("restore", correlation_id);
    let cfg = load_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] restore_version_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;
    validate(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "[cid={}] restore_version_cmd config validation failed: {}",
                cid, e
            ),
        )
    })?;

    let mode = match args.mode {
        RestoreModeDto::InPlace => RestoreMode::InPlace,
        RestoreModeDto::ToDirectory => RestoreMode::ToDirectory,
    };
    let req = RestoreRequest {
        source_path: PathBuf::from(args.source_path),
        version_id: args.version_id,
        mode,
        target_dir: args.target_dir.map(PathBuf::from),
    };
    let result = restore_version(&cfg, &req).map_err(|e| {
        ErrorEnvelope::new(
            "RESTORE_FAILED",
            format!("[cid={}] restore_version_cmd failed: {}", cid, e),
        )
    })?;
    Ok(RestoreResultDto {
        files_written: result.files_written,
        files_removed: result.files_removed,
        dirs_created: result.dirs_created,
    })
}

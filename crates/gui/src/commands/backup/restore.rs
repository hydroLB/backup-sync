use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use backup_core::backup::versioned::restore::{
    list_version_files, list_versions, restore_files, restore_version, ListVersionFilesResult,
    RestoreFilesRequest, RestoreMode, RestoreRequest, VersionFileInfo,
};
use backup_core::load_validated_config;
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

#[derive(Debug, Serialize)]
pub struct VersionFileInfoDto {
    pub rel_path: String,
    pub len: u64,
    pub mtime_unix: i64,
    pub mtime_nanos: u32,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ListVersionFilesResultDto {
    pub total_files: usize,
    pub files: Vec<VersionFileInfoDto>,
}

#[derive(Debug, Deserialize)]
pub struct ListVersionFilesArgs {
    pub source_path: String,
    pub version_id: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RestoreFilesArgs {
    pub source_path: String,
    pub version_id: String,
    pub rel_paths: Vec<String>,
    pub mode: RestoreModeDto,
    pub target_dir: Option<String>,
}

#[tauri::command]
pub async fn list_versions_cmd(
    correlation_id: Option<String>,
) -> Result<Vec<FolderVersionsDto>, ErrorEnvelope> {
    let cid = correlation::cid("restore_list", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || list_versions_blocking(task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "list_versions_cmd", error))?
}

fn list_versions_blocking(cid: String) -> Result<Vec<FolderVersionsDto>, ErrorEnvelope> {
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] list_versions_cmd failed to load config: {}",
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
pub async fn list_version_files_cmd(
    args: ListVersionFilesArgs,
    correlation_id: Option<String>,
) -> Result<ListVersionFilesResultDto, ErrorEnvelope> {
    let cid = correlation::cid("restore_files_list", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || list_version_files_blocking(args, task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "list_version_files_cmd", error))?
}

fn list_version_files_blocking(
    args: ListVersionFilesArgs,
    cid: String,
) -> Result<ListVersionFilesResultDto, ErrorEnvelope> {
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] list_version_files_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;

    let limit = args.limit.unwrap_or(200);
    let ListVersionFilesResult { total_files, files } = list_version_files(
        &cfg,
        PathBuf::from(args.source_path).as_path(),
        args.version_id.as_str(),
        args.query.as_deref(),
        limit,
    )
    .map_err(|e| {
        ErrorEnvelope::new(
            "LIST_FILES_FAILED",
            format!("[cid={}] list_version_files_cmd failed: {}", cid, e),
        )
    })?;

    Ok(ListVersionFilesResultDto {
        total_files,
        files: files
            .into_iter()
            .map(
                |VersionFileInfo {
                     rel_path,
                     len,
                     mtime_unix,
                     mtime_nanos,
                     sha256,
                 }| {
                    VersionFileInfoDto {
                        rel_path,
                        len,
                        mtime_unix,
                        mtime_nanos,
                        sha256,
                    }
                },
            )
            .collect(),
    })
}

#[tauri::command]
pub async fn restore_version_cmd(
    args: RestoreArgs,
    correlation_id: Option<String>,
) -> Result<RestoreResultDto, ErrorEnvelope> {
    let cid = correlation::cid("restore", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || restore_version_blocking(args, task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "restore_version_cmd", error))?
}

fn restore_version_blocking(
    args: RestoreArgs,
    cid: String,
) -> Result<RestoreResultDto, ErrorEnvelope> {
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] restore_version_cmd failed to load config: {}",
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

#[tauri::command]
pub async fn restore_files_cmd(
    args: RestoreFilesArgs,
    correlation_id: Option<String>,
) -> Result<RestoreResultDto, ErrorEnvelope> {
    let cid = correlation::cid("restore_files", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || restore_files_blocking(args, task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "restore_files_cmd", error))?
}

fn restore_files_blocking(
    args: RestoreFilesArgs,
    cid: String,
) -> Result<RestoreResultDto, ErrorEnvelope> {
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] restore_files_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;

    let mode = match args.mode {
        RestoreModeDto::InPlace => RestoreMode::InPlace,
        RestoreModeDto::ToDirectory => RestoreMode::ToDirectory,
    };
    let req = RestoreFilesRequest {
        source_path: PathBuf::from(args.source_path),
        version_id: args.version_id,
        rel_paths: args.rel_paths,
        mode,
        target_dir: args.target_dir.map(PathBuf::from),
    };

    let result = restore_files(&cfg, &req).map_err(|e| {
        ErrorEnvelope::new(
            "RESTORE_FILES_FAILED",
            format!("[cid={}] restore_files_cmd failed: {}", cid, e),
        )
    })?;
    Ok(RestoreResultDto {
        files_written: result.files_written,
        files_removed: result.files_removed,
        dirs_created: result.dirs_created,
    })
}

fn blocking_task_error(cid: &str, command: &str, error: tokio::task::JoinError) -> ErrorEnvelope {
    ErrorEnvelope::new(
        "BLOCKING_TASK_FAILED",
        format!("[cid={cid}] {command} blocking task failed: {error}"),
    )
}

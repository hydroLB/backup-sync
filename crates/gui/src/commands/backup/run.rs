use crate::commands::error::ErrorEnvelope;
use crate::commands::{auth::SessionAuth, security};
use backup_core::{
    backup::versioned, load_config, platform::paths, state::store::StateStore, validate,
};
use fs2::free_space;
use serde::Serialize;

/// Purpose: Loads config and validates it with consistent error envelopes.
///
/// Inputs: the correlation id string.
/// Outputs: a validated config or an error envelope.
/// Ties to: run and simulate commands.
/// Side effects: Reads config from disk and may write defaults during load.
/// Why: centralize config loading and validation error handling.
fn load_and_validate_config(cid: &str) -> Result<backup_core::Config, ErrorEnvelope> {
    let cfg = load_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] load_and_validate_config failed to load config: {}",
                cid, e
            ),
        )
    })?;
    validate(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "[cid={}] load_and_validate_config config validation failed: {}",
                cid, e
            ),
        )
    })?;
    Ok(cfg)
}

/// Purpose: Loads the persisted daemon state store for GUI actions.
///
/// Inputs: the config and correlation id string.
/// Outputs: a tuple of state store and loaded state.
/// Side effects: Reads state from disk.
/// Error handling: Wraps IO failures with a correlation-id tagged envelope.
/// Ties to other methods: Used by `run_now_cmd` for state persistence.
/// Why this exists: centralize state loading and ensure safe-mode stays in sync with config.
fn load_state_store(
    cfg: &backup_core::Config,
    cid: &str,
) -> Result<(StateStore, backup_core::state::StoredState), ErrorEnvelope> {
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "[cid={}] load_state_store failed to resolve state path: {}",
                cid, e
            ),
        )
    })?;
    let (mut state, store) = StateStore::load_or_default(state_path).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!("[cid={}] load_state_store failed to load state: {}", cid, e),
        )
    })?;
    state.safe_mode = cfg.safe_mode;
    Ok((store, state))
}

#[tauri::command]
/// Purpose: Runs a backup immediately from the GUI.
///
/// Inputs: an optional correlation id and session auth state.
/// Outputs: `Ok(())` when the backup completes or an error envelope.
/// Ties to: GUI run now actions and state persistence.
/// Side effects: Reads config/state, performs backup IO, and writes state updates.
/// Why: provide an on demand run path for the UI.
pub async fn run_now_cmd(
    correlation_id: Option<String>,
    auth_state: tauri::State<'_, SessionAuth>,
) -> Result<(), ErrorEnvelope> {
    security::ensure_unlocked(&auth_state, correlation_id.clone())?;
    let cid = security::cid("run", correlation_id);
    eprintln!("[cid={}] run_now start", cid);
    let cfg = load_and_validate_config(&cid)?;
    if cfg.safe_mode {
        return Err(ErrorEnvelope::new(
            "SAFE_MODE",
            format!(
                "[cid={}] Safe mode enabled: skipping writes. Use Simulate or Verify instead.",
                cid
            ),
        ));
    }
    if let Some(min_free) = cfg.min_free_space_bytes {
        match free_space(&cfg.backup_root) {
            Ok(free) if free < min_free => {
                return Err(ErrorEnvelope::new(
                    "FREE_SPACE_LOW",
                    format!(
                        "[cid={}] run_now_cmd free space {} below configured minimum {} at {:?}",
                        cid, free, min_free, cfg.backup_root
                    ),
                ));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(ErrorEnvelope::new(
                    "FREE_SPACE_READ",
                    format!("[cid={}] run_now_cmd failed to read free space: {}", cid, e),
                ))
            }
        }
    }
    let (store, mut state) = load_state_store(&cfg, &cid)?;
    let result = versioned::run_backup_cycle(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "EXEC_FAILED",
            format!("[cid={}] run_now_cmd versioned backup failed: {}", cid, e),
        )
    })?;
    state.last_run_ts = Some(chrono::Utc::now().timestamp());
    state.last_files_backed_up = result.versions_created;
    state.last_error = None;
    store.persist(&state).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_SAVE",
            format!("[cid={}] run_now_cmd failed to persist state: {}", cid, e),
        )
    })?;
    eprintln!(
        "[cid={}] run_now complete versions_created={}",
        cid, result.versions_created
    );
    Ok(())
}

#[derive(Serialize)]
pub struct SimulationResult {
    pub items: usize,
    pub bytes: u64,
    pub sample: Vec<String>,
    pub message: String,
}

#[tauri::command]
/// Purpose: Runs a simulation that reports what would be backed up without writing.
///
/// Inputs: an optional correlation id string.
/// Outputs: a `SimulationResult` or error envelope.
/// Ties to: GUI simulate actions.
/// Side effects: Reads config/state and scans filesystem metadata.
/// Why: allow users to inspect changes before running a backup.
pub async fn run_simulate_cmd(
    correlation_id: Option<String>,
) -> Result<SimulationResult, ErrorEnvelope> {
    let cid = security::cid("sim", correlation_id);
    eprintln!("[cid={}] simulate start", cid);
    Ok(SimulationResult {
        items: 0,
        bytes: 0,
        sample: vec![],
        message: "Simulation is not available in the simplified versioned-backup engine.".into(),
    })
}

use crate::commands::error::ErrorEnvelope;
use crate::commands::{auth::SessionAuth, security};
use backup_core::{
    backup::{execution::BackupExecutor, planning},
    fs::scanning::collect_targets,
    load_config,
    platform::paths,
    state::store::StateStore,
    validate,
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

/// Purpose: Builds a backup plan and loads state and store for execution.
///
/// Inputs: the config and correlation id string.
/// Outputs: a tuple of plan, state store, and loaded state.
/// Ties to: run and simulate commands and state persistence.
/// Side effects: Reads state from disk and scans filesystem metadata.
/// Why: centralize plan creation and related state loading.
fn build_plan(
    cfg: &backup_core::Config,
    cid: &str,
) -> Result<
    (
        planning::BackupPlan,
        StateStore,
        backup_core::state::StoredState,
    ),
    ErrorEnvelope,
> {
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "[cid={}] build_plan failed to resolve state path: {}",
                cid, e
            ),
        )
    })?;
    let (mut state, store) = StateStore::load_or_default(state_path).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!("[cid={}] build_plan failed to load state: {}", cid, e),
        )
    })?;
    let targets = collect_targets(cfg).map_err(|e| {
        ErrorEnvelope::new(
            "SCAN_FAILED",
            format!(
                "[cid={}] build_plan failed to scan watched paths: {}",
                cid, e
            ),
        )
    })?;
    let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing).map_err(|e| {
        ErrorEnvelope::new(
            "PLAN_FAILED",
            format!(
                "[cid={}] build_plan failed to build backup plan: {}",
                cid, e
            ),
        )
    })?;
    if let Err(e) = planning::enforce_plan_limits(&plan, &cfg.planning) {
        return Err(ErrorEnvelope::new(
            "PLAN_TOO_LARGE",
            format!("[cid={}] build_plan plan too large: {}", cid, e),
        ));
    }
    Ok((plan, store, state))
}

/// Purpose: Executes a prepared plan and updates state with results.
///
/// Inputs: the config, plan, mutable state, and correlation id.
/// Outputs: `Ok(())` when execution completes.
/// Ties to: run command execution.
/// Side effects: Performs filesystem IO and mutates stored state.
/// Why: centralize execution error handling for the run command.
fn execute_plan(
    cfg: &backup_core::Config,
    plan: &[planning::PlannedItem],
    state: &mut backup_core::state::StoredState,
    cid: &str,
) -> Result<(), ErrorEnvelope> {
    let exec = BackupExecutor::from_config(cfg);
    exec.execute(plan, state).map_err(|e| {
        ErrorEnvelope::new(
            "EXEC_FAILED",
            format!("[cid={}] execute_plan backup execution failed: {}", cid, e),
        )
    })?;
    Ok(())
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
    let (plan, store, mut state) = build_plan(&cfg, &cid)?;
    if plan.is_empty() {
        eprintln!("[cid={}] run_now no changes; exiting", cid);
        return Ok(());
    }
    execute_plan(&cfg, &plan, &mut state, &cid)?;
    store.persist(&state).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_SAVE",
            format!("[cid={}] run_now_cmd failed to persist state: {}", cid, e),
        )
    })?;
    eprintln!("[cid={}] run_now complete backed_up={}", cid, plan.len());
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
    let cfg = load_and_validate_config(&cid)?;
    let (plan, _, state) = build_plan(&cfg, &cid)?;
    if plan.is_empty() {
        eprintln!("[cid={}] simulate no changes", cid);
        return Ok(SimulationResult {
            items: 0,
            bytes: 0,
            sample: vec![],
            message: "No changes detected; nothing to copy.".into(),
        });
    }
    let bytes: u64 = plan.iter().map(|p| p.len).sum();
    let sample = plan
        .iter()
        .take(cfg.runtime.simulation_sample_limit)
        .map(|p| p.src.display().to_string())
        .collect();
    // Ensure safe_mode short-circuit is reflected
    if cfg.safe_mode || state.safe_mode {
        eprintln!(
            "[cid={}] simulate safe_mode active; no writes would occur",
            cid
        );
    }
    eprintln!(
        "[cid={}] simulate summary items={} bytes={}",
        cid,
        plan.len(),
        bytes
    );
    Ok(SimulationResult {
        items: plan.len(),
        bytes,
        sample,
        message: format!("Would back up {} items ({} bytes).", plan.len(), bytes),
    })
}

use anyhow::{Context, Result};
use backup_core::{
    backup::{execution::BackupExecutor, planning},
    fs::scanning::collect_targets,
    load_config,
    platform::paths,
    state::store::StateStore,
    validate,
};
use fs2::free_space;

/// Purpose: Executes a one off backup run with optional dry run mode.
///
/// Inputs: a dry run flag.
/// Outputs: `Ok(())` when the operation completes.
/// Ties to: CLI command handling and backup execution.
/// Side effects: Reads config/state, scans files, and writes backups when not dry run.
/// Why: allow manual execution of backup cycles from the CLI.
pub async fn run_once(dry_run: bool) -> Result<()> {
    let cfg = load_config().context("cli::run_once failed to load config")?;
    validate(&cfg).context("cli::run_once config validation failed")?;
    if cfg.safe_mode {
        anyhow::bail!(
            "cli::run_once safe mode enabled: scan or verify only. Disable safe mode to run backups."
        );
    }
    // Pre-flight free space check
    if let Some(min_free) = cfg.min_free_space_bytes {
        match free_space(&cfg.backup_root) {
            Ok(free) if free < min_free => {
                anyhow::bail!(
                    "cli::run_once free space {} below configured minimum {} at {:?}",
                    free,
                    min_free,
                    cfg.backup_root
                );
            }
            Ok(_) => {}
            Err(e) => anyhow::bail!(
                "cli::run_once failed to read free space at {:?}: {}",
                cfg.backup_root,
                e
            ),
        }
    }
    let state_path =
        paths::state_file_path().context("cli::run_once failed to resolve state path")?;
    let (mut state, store) = StateStore::load_or_default(state_path)
        .context("cli::run_once failed to load state file")?;
    if state.safe_mode {
        anyhow::bail!(
            "cli::run_once safe mode enabled in state: scan or verify only. Disable safe mode to run backups."
        );
    }
    let targets = collect_targets(&cfg)
        .context("cli::run_once failed to scan watched targets (check watched paths exist)")?;
    let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing)
        .context("cli::run_once failed to build backup plan")?;
    backup_core::backup::planning::enforce_plan_limits(&plan, &cfg.planning)
        .context("cli::run_once backup plan too large")?;
    if plan.is_empty() {
        println!("No changes detected; nothing to back up.");
        return Ok(());
    }
    let exec = BackupExecutor::from_config(&cfg);
    if dry_run {
        let total_bytes: u64 = plan.iter().map(|p| p.len).sum();
        println!(
            "Dry run: {} items would be backed up ({} bytes).",
            plan.len(),
            total_bytes
        );
        return Ok(());
    }
    let res = exec
        .execute(&plan, &mut state)
        .context("cli::run_once backup execution failed")?;
    store
        .persist(&state)
        .context("cli::run_once failed to persist state after backup")?;
    println!("backed up {}", res.backed_up);
    Ok(())
}

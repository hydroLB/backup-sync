use anyhow::{Context, Result};
use backup_core::{
    load_config, platform::paths, state::store::StateStore, verify_backups as core_verify,
};

/// Purpose: Verifies backups by rehashing the most recent copies.
///
/// Inputs: none.
/// Outputs: `Ok(())` when verification completes without issues.
/// Ties to: CLI verification command and state persistence.
/// Side effects: Reads config/state, hashes backup files, and writes updated state.
/// Why: provide a manual integrity check from the CLI.
pub async fn verify_backups() -> Result<()> {
    let cfg = load_config().context("cli::verify_backups failed to load config")?;
    let state_path =
        paths::state_file_path().context("cli::verify_backups failed to resolve state path")?;
    let (mut state, store) = StateStore::load_or_default(state_path)
        .context("cli::verify_backups failed to load state")?;
    if state.files.is_empty() {
        println!("No backups recorded yet.");
        return Ok(());
    }
    let (ok, bad) = core_verify(&mut state, &cfg.hashing)
        .context("cli::verify_backups failed during verification")?;
    store
        .persist(&state)
        .context("cli::verify_backups failed to persist state")?;
    println!("Verification complete: {ok} ok, {bad} issues");
    if bad > 0 {
        anyhow::bail!("cli::verify_backups detected {bad} backup integrity issues");
    }
    Ok(())
}

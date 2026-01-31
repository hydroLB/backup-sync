use anyhow::{Context, Result};
use backup_core::{
    load_config, platform::paths, state::index::BackupIndex, state::store::StateStore,
};

/// Purpose: Prints a snapshot of the current state and config summary.
///
/// Inputs: none.
/// Outputs: `Ok(())` after printing summary lines.
/// Ties to: CLI status command output.
/// Side effects: Reads config/state from disk and writes to stdout.
/// Why: provide a quick operational snapshot for users.
pub fn status() -> Result<()> {
    let cfg = load_config().context("cli::status failed to load config")?;
    let state_path =
        paths::state_file_path().context("cli::status failed to resolve state path")?;
    let (state, _) =
        StateStore::load_or_default(state_path).context("cli::status failed to load state")?;
    let index = BackupIndex::from_state(&state);
    println!("last run: {:?}", state.last_run_ts);
    println!("tracked files: {}", index.file_count());
    println!("backup copies: {}", index.backup_count());
    println!("config root: {:?}", cfg.backup_root);
    Ok(())
}

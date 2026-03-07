use anyhow::{Context, Result};
use backup_core::{
    backup::versioned, load_validated_config, platform::paths, state::store::StateStore,
};
use chrono::Utc;

/// Summary: Verifies backups by rehashing the most recent copies.
///
/// Inputs: none.
///
/// Outputs: `Ok(())` when verification completes without issues.
///
/// Side effects: Reads config/state, hashes backup files, and writes updated state.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI verification command and state persistence.
///
/// Why this exists: provide a manual integrity check from the CLI.
pub async fn verify_backups() -> Result<()> {
    let cfg = load_validated_config().context("cli::verify_backups failed to load config")?;
    let state_path =
        paths::state_file_path().context("cli::verify_backups failed to resolve state path")?;
    let (mut state, store) = StateStore::load_or_default(state_path)
        .context("cli::verify_backups failed to load state")?;
    let now = Utc::now().timestamp();
    let res = versioned::scrub_versioned_store(
        &cfg,
        &cfg.hashing,
        versioned::ScrubMode::Full,
        cfg.runtime.scrub_sample_blobs,
        cfg.runtime.scrub_sample_versions_per_source,
        now as u64,
    )
    .context("cli::verify_backups failed during scrub")?;
    let bad = res.hash_mismatches + res.missing_blobs + res.manifests_bad;
    let ok = res.blobs_hashed.saturating_sub(res.hash_mismatches);
    state.last_verify_ts = Some(now);
    state.last_verify_issues = Some(bad);
    state.last_verify_status = Some(if bad == 0 {
        "ok (full)".into()
    } else {
        "issues_detected (full)".into()
    });
    state.last_scrub_full_ts = Some(now);
    store
        .persist(&state)
        .context("cli::verify_backups failed to persist state")?;
    println!(
        "Scrub complete: mode=full hashed={} ok={} missing={} mismatches={} manifest_issues={}",
        res.blobs_hashed, ok, res.missing_blobs, res.hash_mismatches, res.manifests_bad
    );
    if bad > 0 {
        anyhow::bail!("cli::verify_backups detected {bad} backup integrity issues");
    }
    Ok(())
}

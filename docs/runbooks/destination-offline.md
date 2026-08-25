# Runbook: Destination Offline

## Trigger
- UI or status output reports destination unavailable or destination paused.
- Backups are skipped while source scanning still runs.

## Immediate Safety Actions
1. Do not delete destination data manually.
2. Keep daemon running unless it is actively thrashing.
3. Record current status before any remediation.

## Diagnostics Flow
1. Capture current service and backup status.
   - `cargo run -p cli -- status`
   - `cargo run -p cli -- doctor`
2. Validate destination path currently configured.
   - Check `backup_root` and `[[destinations]]` in the OS-native `config.toml`; see [Configuration](../configuration.md#file-locations).
3. Validate OS-level mount availability and permissions for each destination path.
4. Review recent daemon logs for destination pause reason.
5. Confirm free space is above configured minimum (`min_free_space_bytes` if set).

## Recovery Steps
1. Reconnect or remount the destination device.
2. Confirm path exists and is writable by the daemon user.
3. Re-run status and doctor commands.
4. Trigger one dry-run cycle to confirm planning can proceed.
   - `cargo run -p cli -- run-once --dry-run`
5. If healthy, run one real cycle.
   - `cargo run -p cli -- run-once`

## Rollback Plan
Use this when post-fix behavior is worse than pre-fix behavior.

1. Revert config changes made during remediation using your saved pre-incident config copy.
2. Re-enable safe mode to prevent writes until destination state is revalidated.
   - Set `safe_mode = true` in config, then restart daemon.
3. If a new mount point was introduced, return to the last known-good mount path.
4. Re-run doctor and status; keep safe mode enabled until output is stable.

## Exit Criteria
- `status` no longer reports destination unavailable or paused.
- One dry run and one real run complete without destination errors.
- No new destination-related errors in recent logs.

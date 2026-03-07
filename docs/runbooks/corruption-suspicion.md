# Runbook: Corruption Suspicion

## Trigger
- Verify reports hash mismatch or missing blobs for previously healthy versions.
- Restore outputs data that does not match expected file content.
- Operators suspect destination media errors or silent data corruption.

## Immediate Safety Actions
1. Freeze write activity immediately.
   - Set `safe_mode = true` and restart daemon.
2. Preserve evidence:
   - Save verify output, status output, and daemon logs.
3. Do not run manual cleanup or garbage-collection actions.

## Diagnostics Flow
1. Capture current state.
   - `cargo run -p cli -- status`
   - `cargo run -p cli -- doctor`
   - `cargo run -p cli -- verify`
2. Identify first known-bad version timestamp and affected destination IDs.
3. Check destination hardware and filesystem health outside the app.
4. Validate whether corruption is isolated:
   - single watched path
   - single destination
   - multiple unrelated blobs
5. If replication is enabled, compare primary and mirror behavior.

## Recovery Steps
1. Restore critical data from known-good versions to a quarantine location.
2. If mirror destination is healthy, use mirror as source of truth for restores.
3. Replace or repair failing destination hardware before resuming writes.
4. Re-run verify until issue count is zero.
5. Only then disable safe mode and resume scheduled backup cycles.

## Rollback Plan
1. Keep safe mode enabled if any post-remediation verify run fails.
2. Revert destination/path changes made during remediation.
3. Return to last known-good destination and config combination.
4. Resume in dry-run mode first:
   - `cargo run -p cli -- run-once --dry-run`
5. Promote to real runs only after two clean verify passes.

## Exit Criteria
- Two consecutive verify runs are clean.
- Destination health checks pass.
- Safe mode can be disabled without new integrity failures.

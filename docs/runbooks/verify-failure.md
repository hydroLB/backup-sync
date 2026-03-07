# Runbook: Verify Failure

## Trigger
- `cargo run -p cli -- verify` reports integrity issues.
- UI verify block shows failed or warning status.

## Immediate Safety Actions
1. Stop non-essential writes to avoid masking evidence.
2. Keep a copy of current logs and verify output before retrying.
3. Avoid pruning backup history during investigation.

## Diagnostics Flow
1. Capture baseline status.
   - `cargo run -p cli -- status`
   - `cargo run -p cli -- doctor`
2. Execute verify and save full output.
   - `cargo run -p cli -- verify`
3. Classify failure type:
   - Missing blob
   - Hash mismatch
   - Read/access error
4. Scope blast radius:
   - Identify affected watched paths, versions, and destination IDs.
5. Validate destination filesystem health and read consistency.

## Recovery Steps
1. If failure is read/access only, repair filesystem or permissions and rerun verify.
2. If failure is missing blob or hash mismatch:
   - Restore affected files from last known-good version to a quarantine path.
   - Mark incident as potential corruption and move to corruption runbook.
3. Run dry-run backup to ensure planner/executor are stable.
   - `cargo run -p cli -- run-once --dry-run`
4. Run verify again and confirm issue count is zero.

## Rollback Plan
1. If remediation changed config/tuning, restore pre-incident config.
2. If recent backups after remediation increase failures, disable writes temporarily:
   - set `safe_mode = true`
3. Restore user data from last known-good version snapshots into a new directory.
4. Defer in-place restores until verify is consistently clean.

## Exit Criteria
- Verify completes with no issues.
- Status shows no new `last_error`.
- Recovery actions and affected versions are documented.

# Runbook: Stuck Daemon

## Trigger
- UI stops receiving fresh status updates.
- Scheduled cycles do not advance for longer than expected interval.
- IPC checks time out or daemon process stays alive but non-responsive.

## Immediate Safety Actions
1. Capture diagnostics before restart so root-cause signals are preserved.
2. Avoid repeated force-kill loops that hide timing-related failures.
3. If backup writes are active and risky, enable safe mode before restart.

## Diagnostics Flow
1. Capture command output:
   - `cargo run -p cli -- status`
   - `cargo run -p cli -- doctor`
2. Check process liveness and recent logs.
3. Identify whether issue is:
   - IPC only
   - scheduler/cycle loop stall
   - external IO stall (destination, filesystem, permissions)
4. Validate timeout-related config in `[runtime]`, `[execution]`, `[hashing]`.
5. Check whether shutdown join timeout had prior warnings in logs.

## Recovery Steps
1. Perform controlled daemon restart via normal service mechanism.
2. Re-check status and doctor immediately after restart.
3. Run dry-run cycle to validate pipeline responsiveness.
   - `cargo run -p cli -- run-once --dry-run`
4. If responsive, run one real cycle.
   - `cargo run -p cli -- run-once`
5. Monitor two schedule intervals for sustained progress.

## Rollback Plan
1. If restart + config tuning worsens behavior, revert tuning to pre-incident values.
2. Restore previous service configuration/manifest if changed during remediation.
3. Keep daemon in safe mode if responsiveness is unstable.
4. Fall back to manual `run-once --dry-run` diagnostics until root cause is isolated.

## Exit Criteria
- IPC status calls are responsive.
- Scheduled cycles advance normally across two intervals.
- No recurring stall signatures in logs after restart.

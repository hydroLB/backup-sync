# Security Runbook

Last updated: 2026-08-23

## Local commands

Run from the repository root:

```bash
make secrets
make lockfile-check
make deny
make audit
make security-check
```

`make security-check` combines the working-tree/history secret scan, lockfile validation, Rust supply-chain policy, RustSec audit, and full npm audit at high severity.

## Secret finding

1. Determine whether the value is a real credential.
2. If real, revoke/rotate it first, remove it from files and history as appropriate, and document the incident privately.
3. If false positive, add the narrowest justified `.gitleaks.toml` rule; never blanket-ignore broad source directories.
4. Re-run both working-tree and history scans.

## Lockfile finding

Regenerate through the normal dependency manager, review the resolved diff, then run:

```bash
make lockfile-check
make audit
make deny
```

Do not edit checksums by hand or accept an unexpected source URL.

## Advisory or policy finding

1. Upgrade or replace the dependency when possible.
2. Trace whether the vulnerable code is reachable in the compiled feature graph.
3. Treat an ignore as temporary exceptional policy requiring a linked owner, rationale, scope, and removal condition.
4. Update the threat model/risk register if the dependency cannot be removed promptly.

There is currently no `[advisories].ignore` inventory in `deny.toml`. Current scans report zero vulnerabilities. The 19 `cargo deny` messages from the Tauri/Linux GTK3 and HTML dependency graph are informational warnings; do not describe them as ignored advisories or resolved maintenance risk.

## Suspected store tampering or corruption

1. Stop backup/replication writes and preserve the destination.
2. Capture redacted diagnostics without moving or deleting store metadata.
3. Follow [Corruption suspicion](runbooks/corruption-suspicion.md) and [Verify failure](runbooks/verify-failure.md).
4. Restore to a new directory first; do not overwrite the only source of evidence.
5. Record affected manifests, hashes, encryption key ID, and software commit without including key material.

## Security-impacting change record

Include trigger, affected boundary, exploit/failure condition, tests, mitigation, residual risk, and follow-up owner. Private reports follow [SECURITY.md](../SECURITY.md).

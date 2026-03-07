# Incident Runbooks

## Purpose
This directory contains operator-facing incident playbooks for common backup service failures.

## Runbook Index
1. Destination offline: [`docs/runbooks/destination-offline.md`](destination-offline.md)
2. Verify failure: [`docs/runbooks/verify-failure.md`](verify-failure.md)
3. Corruption suspicion: [`docs/runbooks/corruption-suspicion.md`](corruption-suspicion.md)
4. Stuck daemon: [`docs/runbooks/stuck-daemon.md`](stuck-daemon.md)

## Standard Response Flow
1. Stabilize: avoid actions that can destroy evidence or healthy backups.
2. Diagnose: capture status, logs, and the smallest reproducible failure signal.
3. Contain: limit blast radius (safe mode, pause writes, isolate destination).
4. Recover: use the incident-specific recovery path.
5. Roll back: return to last known-good state if recovery changes increase risk.
6. Record: capture timeline, root cause hypothesis, and follow-up work.

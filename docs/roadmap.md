# Roadmap

Last updated: 2026-08-23

Backup Sync is pre-1.0 and source-distributed. The next milestones reduce recovery and distribution risk rather than add novelty.

## Delivered engineering baseline

- Versioned content-addressed storage with retention, garbage collection, compression, encryption, scrub, replication, and restore.
- Exact content/hash publication invariant, strict untrusted-metadata validation, staged verified restore, and per-store leases.
- Self-healing replication, terminating sampled scrub, atomic state replacement, and private singleton IPC on Unix.
- Daemon, CLI, Tauri/React app, service definitions, diagnostics, health/readiness, structured logging, and runbooks.
- Canonical tests/build/coverage/performance/security gate, CodeQL, and macOS/Windows workspace compilation.

## Milestone 1: releasable artifacts

Reduces R-02 and R-10.

- Produce versioned Tauri, CLI, and daemon artifacts from signed tags.
- Add checksums and provenance.
- Configure macOS signing/notarization and Windows signing without repository credentials.
- Run clean-machine install, upgrade, service registration, backup, verify, restore, and uninstall tests.

Done means a tagged release produces verifiable artifacts for every claimed platform and each passes a clean-machine recovery smoke test.

## Milestone 2: cross-process and platform hardening

Reduces R-03, R-04, and R-05.

- Define/test per-user Windows named-pipe ACLs and singleton cleanup.
- Add a store/state transaction protocol for cross-process state updates.
- Add cooperative cancellation checkpoints to long filesystem operations.
- Exercise shutdown during scan, blob write, restore staging, scrub, and replication.

## Milestone 3: retire the legacy engine

Reduces R-06.

- Inventory remaining consumers of legacy scan-plan-copy exports.
- Publish migration examples and deprecation notices.
- Move remaining behavior coverage to the versioned engine.
- Remove exports only in the compatible release window defined by policy.

## Milestone 4: recovery depth and UX

Reduces R-01, R-07, R-08, R-11, and R-13.

- Require and verify an encryption-key recovery copy.
- Design authenticated manifest/index metadata before claiming malicious-tamper protection.
- Add interruption and property tests for storage/restore invariants.
- Surface backup/restore progress, replication lag, scrub freshness, and safe cancellation boundaries.
- Separate basic scheduling from file-driven advanced scheduling.

## Non-goals before 1.0

- Hosted accounts or a proprietary cloud brokerage service
- Mobile clients
- Block-level filesystem deduplication
- Reimplementing mature cryptographic, compression, watcher, or desktop-framework dependencies

See the [risk register](risk-register.md) for controls and residuals.

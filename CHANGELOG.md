# Changelog

All notable changes to Backup Sync are documented here in Keep a Changelog style. The project follows Semantic Versioning.

No release tags exist yet. Version `0.1.0` remains the planned source-distributed baseline.

## [Unreleased]

### Added

- Strict restore metadata validation for version IDs, manifest/index coherence, relative paths, and lowercase SHA-256 digests.
- Cross-process store leases covering versioned backup, restore, scrub, retention, garbage collection, and replication.
- Replica repair for already-indexed versions and validation-before-publication semantics.
- Atomic state-file persistence, bounded blocking-work adapters, and private per-user singleton IPC.
- Frontend error boundary, recoverable config-load errors, stale-request protection in restore search, and accessibility/reduced-motion improvements.
- Cross-platform service-manifest escaping tests and macOS/Windows workspace compilation jobs.
- Repository-local documentation, release-permission, launcher-consistency, architecture, operability, hygiene, lockfile, and CI-contract checks.

### Changed

- Enforced the content-address invariant: live source mutation cannot publish content under a digest it no longer matches.
- Hardened restore to decode and hash each blob before a symlink-safe staged tree replaces the target.
- Made sampled scrub choose a coprime stride so every requested sample terminates with the expected cardinality.
- Made GUI/daemon filesystem work leave the async runtime and serialize daemon state commits without holding application-state locks across long I/O.
- Removed implicit backup, config mutation, daemon stop, safe-mode persistence, and forced-exit behavior from GUI close/quit handling.
- Standardized the product identity, Tauri configuration, and fixed development port `5173`.
- Expanded `make check` to include backend tests, builds, coverage, performance, secret scanning, dependency policy, and full Rust/Node audits.
- Updated frontend dependencies so the full npm audit reports zero vulnerabilities.
- Removed the stale Rust advisory ignore inventory; current `cargo deny` output contains informational transitive warnings rather than ignored vulnerabilities.
- Clarified source-only distribution and separated platform compilation from packaging/runtime claims.

### Security

- Prevented manifest path traversal, malformed-hash panics, unsafe diagnostic/export filename collisions, doctor probe truncation, and live IPC endpoint unlinking.
- Added exact-hash checks before blob persistence and before restore target mutation.
- Restricted Unix daemon runtime directories/endpoints and verified socket identity during cleanup.
- Added an explicit Tauri content security policy for production and scoped development exceptions to localhost.

## [0.1.0] - Unreleased baseline

Planned first tagged baseline: versioned content-addressed backups, retention, restore, integrity verification, daemon/CLI/desktop surfaces, performance guards, and release validation. This heading is not evidence of a published release.

# Threat Model

Last updated: 2026-08-23

## Scope and assets

Critical assets are backup availability and integrity, the encryption key, source/destination confidentiality, and trustworthy operational status. Trust boundaries include config/environment input, manifest/index/blob metadata, source/destination filesystems, local IPC clients, platform service definitions, and diagnostic exports.

The system assumes the host account and kernel are not fully compromised. A malicious process running as the same user can read plaintext manifests and may access any file allowed by that account; Backup Sync does not claim remote attestation.

## Store and restore

Threats include manifest path traversal, absolute paths, map-key/path mismatch, malformed hashes, missing or substituted blobs, source mutation during read, symlink races, incomplete replication, and concurrent store mutation.

Controls:

- Strict version-ID, relative-path, key coherence, and lowercase 64-character digest validation
- Exact decoded-plaintext SHA-256 enforcement before blob publication and restore target mutation
- Symlink-safe staged restore and final rename only after complete preflight/reconstruction
- Bounded per-store cross-process leases, ordered deterministically for multi-store operations
- Blob → manifest → index commit ordering; replication repair and source revalidation before replica index publication
- Sampled/full scrub and optional chunked AEAD encryption

Residuals: plaintext manifests expose paths/metadata; a malicious local actor can delete/replace metadata; authenticated manifest/index signing is not implemented; filesystem/mount churn retains a bounded TOCTOU class.

## IPC and process lifecycle

Threats include oversized messages, endpoint impersonation, unlinking a live daemon socket, second-instance races, unsafe cleanup, and local request flooding.

Controls:

- Bounded request/response sizes and timeouts
- Private per-user Unix runtime directory, mode-`0600` socket, endpoint type/owner/identity checks
- Live singleton detection; only a verified refused stale socket is removable
- Cleanup removes only the endpoint identity created by that daemon
- Structured, sanitized request/correlation identifiers

Residuals: same-user valid-request rate limiting is not implemented; explicit Windows pipe ACLs and clean-machine runtime behavior are unverified.

## Config, service definitions, and exports

Threats include environment override confusion, unsafe destination mapping, XML/systemd injection, predictable temporary files, symlink truncation, and path/secret leakage.

Controls:

- One parse/layer/validate flow shared by entrypoints; `BACKUP_SYNC_CONFIG` is coherent for read and write
- Validated destination/source relationships and real-destination free-space checks in core
- Context-aware launchd/Windows XML escaping and systemd argument escaping/control-character rejection
- Exclusive unpredictable doctor/diagnostic/export files with cleanup on incomplete writes
- Stable redacted boundary errors and Tauri content security policy

Residuals: operator misconfiguration remains possible; generated service behavior is not runtime-tested on every platform.

## Supply chain

CI runs secret scanning, lockfile hygiene, `cargo deny`, `cargo audit`, full npm audit, and CodeQL. Current scans report zero vulnerabilities and no Rust advisory ignore list. `cargo deny` emits 19 informational warnings from the transitive Tauri/Linux GTK3 and HTML stack; those upstream maintenance signals remain tracked risks.

## Priority follow-ups

1. Define Windows per-user pipe ACLs and add platform runtime tests.
2. Add authenticated manifest/index metadata.
3. Make cross-process state updates transactional.
4. Add IPC backpressure and cancellation checkpoints for long blocking work.
5. Reassess Tauri/Linux dependency warnings on every desktop-stack upgrade.

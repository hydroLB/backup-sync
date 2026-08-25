# Storage Model

Last updated: 2026-08-23

This project stores backups as versioned manifests that reference immutable file blobs by SHA-256.

## Layout

Destination root:
- `<dest>/.backup_sync/v1/`

Within the store:
- `operation.lock`
  - Per-store cross-process lease used by backup, restore, retention/GC, scrub, and replication.
- `blobs/sha256/<prefix>/<sha256>`
  - Content-addressed blobs; `<prefix>` is the first 2 hex characters of the SHA-256.
- `sources/<source_id>/index.json`
  - Version index for the watched folder.
- `sources/<source_id>/manifests/<version_id>.json`
  - Snapshot manifest for a version.

Where:
- `<source_id>` is `sha256(watched_path_as_string)` rendered as hex.
- `<version_id>` is a timestamp-based id with sub-second precision; collisions are avoided by suffixing when needed.

## Manifest schema

Each manifest is a JSON object:
- `schema_version`: store schema version
- `source_path`: absolute source folder path (string)
- `created_at_unix`: unix timestamp in seconds (i64)
- `source_snapshot`: optional metadata when a snapshot view was used (provider + id)
- `source_snapshot_error`: optional error string when snapshots are enabled but not usable
- `read_failures`: optional list of read failures encountered during the scan or blob write phase
- `entries`: map keyed by relative path string

Entry fields:
- `kind`: `file` or `dir`
- `rel_path`: normalized relative path (`/` separators)
- `len`: file size (bytes)
- `mtime_unix`: unix timestamp seconds
- `mtime_nanos`: sub-second nanoseconds
- `sha256`: hex SHA-256 for files (null for directories)

## Backup behavior

- Scan each watched folder to build a new snapshot manifest.
- Compare to the latest manifest; create a new version only when the snapshot differs.
- Write a blob only when it does not already exist in `blobs/sha256/...`.
- Hash the final decoded plaintext and require it to equal the expected blob key before publication. If a live source mutates during the read, that content is rejected rather than stored under the wrong digest.
- Acquire a bounded destination lease before mutating store state. Multi-destination operations normalize, sort, and acquire leases deterministically.
- Enforce retention per folder; when older manifests are deleted, unreferenced blobs are garbage collected.

## At-rest encryption (blobs only)

When `[encryption].enabled = true`, blob bytes are stored encrypted on disk while manifests remain plaintext:
- Blob filenames still use the plaintext SHA-256 so deduplication and integrity checks remain stable.
- Restores and scrubs decrypt blobs on read using the configured key file and key id.
- Losing the key file makes encrypted backups unrecoverable.

## At-rest compression (blobs only)

When `[compression].enabled = true`, new blobs are stored in a compressed, chunked container:
- Blob filenames still use the plaintext SHA-256 so deduplication remains stable across settings changes.
- Scrub verification remains meaningful because it hashes the **decoded plaintext** (decrypt then decompress).

Compression and encryption can be enabled independently:
- Compression only: blobs start with the magic `BSYNCCMP`.
- Encryption + compression: blobs start with `BSYNCENC` and use an encryption algorithm id that indicates the per-chunk compressed payload.

## Restore behavior

Before preflight, restore treats manifest/index data as untrusted: version IDs must be valid, manifest map keys must agree with entry paths, paths must remain safe and relative, and file hashes must be lowercase 64-character hexadecimal SHA-256 values.

Restore to directory:
- Preflight: verify all referenced blobs exist and check free space before writing anything.
- Decode each blob into a symlink-safe staging path and verify its plaintext SHA-256 before target mutation.
- Build the full restore tree in a staging directory, then rename it into place (transactional swap).
- For restore-to-directory, if the target already exists it is moved aside to a `.backup_sync_restore_prev_*`
  sibling directory so you can recover it if needed.

Restore in place:
- Preflight: verify all referenced blobs exist and check free space before writing anything.
- Decode each blob into a symlink-safe staging path and verify its plaintext SHA-256 before target mutation.
- Build the full restore tree in a staging directory, then swap it into place by renaming the original
  directory aside and renaming the staged tree into the original path.

## Edge cases handled

- Atomic writes: blobs and restored files are written to a same-directory temporary file then renamed into place.
- Crash-consistent commits: blobs are written and fsynced before the manifest is committed, and the manifest is committed before the version index is updated. Parent directories are fsynced where supported.
- Read errors: transient file read failures are retried with backoff; failures are recorded and previous manifest entries are carried forward to avoid treating unreadable paths as deletions.
- Missing blobs: restore fails with a clear list of missing blobs/entries.
- Permissions: IO failures are surfaced with contextual error messages including operation and path.
- Temp files: uses OS temp or per-directory temporary files and relies on RAII cleanup when possible.

## Scan report

Each watched source also maintains:
- `sources/<source_id>/last_scan_report.json`
  - The most recent scan timestamp, optional committed version id, optional snapshot metadata/error, and any `read_failures`.

## Source snapshots (consistent reads)

When enabled, Backup Sync attempts to read the source from a filesystem snapshot so a changing or
glitching source does not produce an inconsistent manifest. Snapshots are best-effort:
- If snapshot creation/mount fails, the cycle continues using the live filesystem view.
- The manifest and `last_scan_report.json` record whether a snapshot was used.

Relevant knobs live under `[runtime]`:
- `source_snapshots_enabled`: enables snapshot-backed reads (default: false).
- `source_snapshot_timeout_seconds`: time bound for snapshot operations (default: 20).

Platform notes:
- macOS: uses `tmutil localsnapshot` plus `mount_apfs -s` to mount a read-only snapshot view.
  This may require elevated privileges depending on the system configuration.
- Windows: uses VSS via PowerShell CIM calls to create a shadow copy and reads files via the
  `GLOBALROOT` device path. This may require administrative privileges.

## Scrub (bit-rot detection)

The daemon runs periodic scrubs to detect destination corruption:
- Fast path: **sampled** scrub
  - Checks the newest N manifests per source (configurable) for blob existence.
  - Re-hashes a bounded number of referenced decoded blobs (configurable).
  - Walks a deterministic permutation with a coprime stride, guaranteeing termination and the requested sample cardinality.
- Slow path: **full** scrub
  - Walks all manifests and re-hashes all referenced blobs.

Relevant knobs live under `[runtime]`:
- `verify_interval_seconds`: cadence for running the scrub task.
- `scrub_full_interval_seconds`: how often the scrub escalates to full mode.
- `scrub_sample_versions_per_source`: how many newest versions per source to inspect in sampled mode.
- `scrub_sample_blobs`: how many blobs to re-hash in sampled mode.

## Replication consistency

- Replication validates every required source manifest and blob before a version becomes visible at the destination.
- Missing destination manifests/blobs are repaired even if the version already appears in the replica index.
- The source is revalidated immediately before the destination index is published.
- Retention mirroring happens only after successful publication and is bounded by `replication_max_manifest_deletes_per_cycle`.
- A replication failure leaves the primary backup committed but surfaces a degraded/failure state and a non-success boundary result.

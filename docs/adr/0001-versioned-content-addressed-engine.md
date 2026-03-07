# ADR 0001: Versioned Content-Addressed Backup Engine

- Status: Accepted
- Date: 2026-02-20

## Context
The system needs efficient repeated backups for mostly unchanged files while preserving historical versions and supporting integrity verification.

## Decision
Use a versioned manifest model backed by SHA-256 content-addressed blobs as the primary storage engine.

## Consequences
- Unchanged file content is not duplicated across versions.
- Integrity verification can re-hash blobs and compare against manifest references.
- Restore can target specific versions with deterministic reconstruction rules.
- Storage and recovery logic stay concentrated in `crates/core/src/backup/versioned`.

## Alternatives considered
- Full-copy snapshots per run (rejected due to storage amplification).
- Block-level delta format (rejected for added complexity at current product stage).

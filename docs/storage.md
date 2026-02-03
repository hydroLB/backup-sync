# Storage model (Blob store + manifests)

This project stores backups as versioned manifests that reference immutable file blobs by SHA-256.

## Layout

Destination root:
- `<dest>/.backup_sync/v1/`

Within the store:
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
- Enforce retention per folder; when older manifests are deleted, unreferenced blobs are garbage collected.

## Restore behavior

Restore to directory:
- Recreate directories and files from the selected manifest, using atomic file writes.

Restore in place:
- First removes files/directories that are not present in the selected manifest.
- Then writes files from the manifest.

## Edge cases handled

- Atomic writes: blobs and restored files are written to a temp file then renamed into place.
- Missing blobs: restore fails with a clear list of missing blobs/entries.
- Permissions: IO failures are surfaced with contextual error messages including operation and path.
- Temp files: uses OS temp or per-directory temporary files and relies on RAII cleanup when possible.


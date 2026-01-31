# Release process

This document describes how to tag, document, and ship releases.

## Versioning
- Use semantic versioning: MAJOR.MINOR.PATCH.
- Increment MAJOR for incompatible config or storage changes.
- Increment MINOR for backward compatible features and new knobs.
- Increment PATCH for fixes and internal improvements.

## Preparation
1. Ensure `CHANGELOG.md` has an Unreleased section with the intended release notes.
2. Run the full quality gate locally:
   - `make build`
   - `make perf-check`
   - `cargo test -p backup_core -p daemon -p cli`
   - `npm --prefix crates/gui/frontend test`
3. Confirm CI is green.

## Tagging
1. Update `CHANGELOG.md` by moving Unreleased entries into a versioned section.
2. Commit the changelog update.
3. Create a signed tag: `git tag -s vX.Y.Z -m "Backup Sync vX.Y.Z"`.
4. Push the tag to publish the release notes.

## Release artifacts
- Desktop app builds are produced via the Tauri pipeline.
- CLI and daemon are built from the Rust workspace and published as versioned binaries.


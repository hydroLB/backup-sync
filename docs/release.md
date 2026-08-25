# Release Process

Last updated: 2026-08-23

No Backup Sync release tags exist yet. The current workflow validates version metadata and publishes GitHub release notes; it does not build, sign, notarize, checksum, or attach application binaries.

## Policy

1. Versions follow SemVer and tags use `vMAJOR.MINOR.PATCH` or `vMAJOR.MINOR.PATCH-rc.N`.
2. Tags must be annotated, point to history reachable from `main`, and have a matching dated changelog section.
3. Releases are cut only after required checks pass.
4. Signed tags are a maintainer requirement, but the hosted workflow currently verifies only that a tag is annotated.
5. Until artifact jobs and clean-machine tests exist, every release is source/metadata-only.

## Current automation

- Release Drafter prepares categorized notes from merged pull requests.
- The tag workflow validates tag syntax, ancestry, changelog content, and its least-privilege permissions.
- Only the publish job receives `contents: write`; validation remains read-only.
- The publish job creates GitHub release metadata with generated notes.

## Local readiness

```bash
make check
./scripts/release/validate_release_ref.sh HEAD origin/main
./scripts/release/validate_changelog_for_release.sh X.Y.Z CHANGELOG.md
./scripts/release/validate_semver_tag.sh vX.Y.Z
```

## Maintainer steps

1. Confirm `main` and all required checks are green.
2. Move relevant Unreleased entries into `## [X.Y.Z] - YYYY-MM-DD`.
3. Run the local readiness commands.
4. Commit the changelog.
5. Create a signed annotated tag: `git tag -s vX.Y.Z -m "Backup Sync vX.Y.Z"`.
6. Push `main`, then push the tag.
7. Verify the workflow publishes notes and clearly labels the release as source-only.

Do not imply that a `0.1.0` changelog baseline is released until the corresponding tag and GitHub release exist.

## Before binary distribution

The release contract must expand to include reproducible Tauri/CLI/daemon artifacts, checksums/provenance, platform signing/notarization, and clean-machine install → backup → verify → restore → uninstall tests. Those items remain roadmap work.

## Corrections

Do not move an existing tag to a different commit. Publish a follow-up patch version and document the correction. Public-contract deprecation rules are in [Deprecation policy](deprecation-policy.md).

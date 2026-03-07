# Release Process

Last updated: 2026-02-20

This document defines the release contract for Backup Sync: how versions are chosen, how tags are validated, and how changelog/release notes are produced consistently.

## Release policy
1. Versioning follows SemVer: `MAJOR.MINOR.PATCH`.
2. Release tags must match `vMAJOR.MINOR.PATCH` (for example `v1.4.2`).
3. Pre-release tags are allowed only as `vMAJOR.MINOR.PATCH-rc.N` (for example `v1.5.0-rc.1`).
4. Release tags must be annotated and signed.
5. Releases are cut from `main` after required CI checks are green.

## SemVer decision table
- MAJOR: incompatible changes to public contracts (`docs/public-api.md`, IPC contract, CLI contract, config contract).
- MINOR: backwards-compatible features, new optional knobs, additive API surface.
- PATCH: bug fixes, internal refactors, and non-breaking operational improvements.

## Changelog and release notes automation
- `CHANGELOG.md` is the canonical human-curated release history.
- GitHub Release Drafter (`.github/workflows/release-drafter.yml`) continuously prepares draft release notes from merged PRs.
- GitHub Release workflow (`.github/workflows/release.yml`) runs on SemVer tags, validates policy, then publishes a GitHub Release with generated release notes.
- Auto-generated release note categories are configured in `.github/release.yml`.
- Manual override template for release notes lives in `docs/templates/release-notes.md`.

## Local release readiness checks
Run before creating a tag:
1. `make check`
2. `./scripts/release/validate_changelog_for_release.sh X.Y.Z CHANGELOG.md`
3. `./scripts/release/validate_semver_tag.sh vX.Y.Z`

## Release steps (maintainer runbook)
1. Confirm `main` is green and branch protection checks passed.
2. Finalize `CHANGELOG.md` with a dated `## [X.Y.Z] - YYYY-MM-DD` section.
3. Run local release checks listed above.
4. Commit the changelog update.
5. Create a signed tag:
   - `git tag -s vX.Y.Z -m "Backup Sync vX.Y.Z"`
6. Push commit and tag:
   - `git push origin main`
   - `git push origin vX.Y.Z`
7. Verify the `Release` workflow completes and publishes notes.

## Rollback/correction policy
- If a tag is invalid or release automation fails, do not retag a different commit with the same tag.
- Create a follow-up patch release (`vX.Y.(Z+1)`) with corrective notes in `CHANGELOG.md`.
- If an emergency unpublish is needed, document the reason in the next changelog entry.

## Public contract deprecation policy
Deprecation rules for API/CLI/IPC/config contracts are defined in [`docs/deprecation-policy.md`](deprecation-policy.md). MAJOR release changes must reference deprecations that were announced before removal.

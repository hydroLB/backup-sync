# Changelog

All notable changes to Backup Sync are documented in this file.

The format follows Keep a Changelog style and the project follows Semantic Versioning.

## [Unreleased]

### Added
- Release automation baseline:
  - release drafter workflow and config
  - SemVer tag validation in release workflow
  - generated GitHub release notes categories
- Deprecation policy for public contracts in `docs/deprecation-policy.md`.

### Changed
- Hardened release policy in `docs/release.md` with SemVer/tag constraints and repeatable maintainer steps.

## [0.1.0] - 2026-02-20

### Added
- Initial documented release process and versioning policy.
- Performance baselines and regression guard checks.
- Strict config validation with centralized defaults and limits.
- End-to-end smoke coverage for the main backup workflow.

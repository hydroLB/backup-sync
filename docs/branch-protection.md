# Branch Protection and Required Checks

Last updated: 2026-08-23

This is the intended policy for `main`; GitHub rulesets enforce it outside the repository.

## Required checks

Require these exact CI jobs:

1. `ci-format`
2. `ci-lint`
3. `ci-typecheck`
4. `ci-tests`
5. `ci-coverage`
6. `ci-perf`
7. `ci-build`
8. `ci-security`
9. `ci-canonical-contract`
10. `ci-platform-compile-macos`
11. `ci-platform-compile-windows`
12. `Analyze (rust)`
13. `Analyze (javascript-typescript)`

If a job is renamed, update this document and the repository's CI-contract smoke test in the same change.

## Merge settings

1. Require pull requests and at least one approval.
2. Require code-owner review and dismiss stale approvals.
3. Require all status checks and an up-to-date branch.
4. Require resolved conversations.
5. Disallow force pushes and branch deletion.

## Canonical local contract

`make check` is the full gate; `make ci` delegates to it. The command includes:

- Rust/frontend format, lint, Rustdoc, and type checks
- architecture, operability, hygiene, documentation, desktop-tool, and lockfile checks
- performance guards, backend tests, workspace/frontend builds, and coverage
- working-tree/history secret scans, `cargo deny`, `cargo audit`, and full npm audit

The `ci-canonical-contract` smoke test prevents workflows from describing a smaller command as canonical.

## Platform confidence boundary

Linux runs the complete hosted gate. macOS and Windows jobs compile the full workspace and all targets/features. They do not produce installers or prove service registration, IPC permissions, native dialogs, upgrade/uninstall, or backup/restore behavior on a clean machine.

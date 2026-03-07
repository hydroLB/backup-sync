# Branch Protection and Required Checks Policy

Last updated: 2026-02-20

## Scope
This policy applies to the `main` branch.

## Required Status Checks
Configure branch protection to require these checks before merge.
The names must match GitHub check run names exactly.

### CI workflow required checks
1. `ci-format`
2. `ci-lint`
3. `ci-typecheck`
4. `ci-tests`
5. `ci-coverage`
6. `ci-build`
7. `ci-security`
8. `ci-canonical-contract`

### SAST workflow required checks
1. `Analyze (rust)`
2. `Analyze (javascript-typescript)`

## Merge Policy (GitHub settings)
Enable these settings for `main`:

1. Require a pull request before merging.
2. Require approvals: minimum `1`.
3. Dismiss stale pull request approvals when new commits are pushed.
4. Require review from Code Owners.
5. Require status checks to pass before merging.
6. Require branches to be up to date before merging.
7. Require conversation resolution before merging.
8. Do not allow force pushes.
9. Do not allow branch deletions.

## Canonical Command Contract
`make ci` is the canonical full-gate command.

- CI job `ci-canonical-contract` validates this contract by checking `make -n ci` output for:
  - format checks
  - lint checks
  - typecheck checks
  - hygiene check
  - secrets scan
  - perf gate
  - coverage gate
  - dependency audits

## Enforcement Notes
1. If a workflow job is renamed, update this file in the same PR.
2. If a new gate is introduced, add it to both:
   - required checks in GitHub branch protection settings
   - this policy document
3. Required checks should stay deterministic and avoid dynamic naming.

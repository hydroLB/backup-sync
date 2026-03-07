# Security Runbook

## Purpose
Operational guidance for running security checks locally and triaging failures before opening a PR.

## Local Security Commands
Run these from repository root:

1. Secret scans (working tree + git history)
   - `make secrets`
2. Lockfile hygiene validation
   - `make lockfile-check`
3. Rust/Node dependency advisories
   - `make audit`
4. Rust supply-chain policy (licenses/sources/advisories/bans)
   - `make deny`
5. Full security gate bundle
   - `make security-check`

## CI Security Gates
The `ci-security` job runs:
1. `gitleaks` action for repository secret scanning.
2. `make lockfile-check`.
3. `make deny`.
4. `make audit`.

## Secret Scan Triage
When a secret scan fails:

1. Confirm whether the finding is a real credential.
2. If real:
   - Revoke and rotate the credential immediately.
   - Remove the credential from current files.
   - Remove from history if needed.
   - Document the incident and follow-up PR.
3. If false positive:
   - Add a narrow allowlist entry in `.gitleaks.toml` (or equivalent policy file).
   - Include rationale and example in PR description.
   - Do not blanket-ignore entire directories without justification.

## Lockfile Hygiene Triage
If `make lockfile-check` fails:

1. Regenerate Rust lockfile via normal dependency command path.
2. Regenerate frontend lockfile with `npm --prefix crates/gui/frontend install --package-lock-only`.
3. Re-run `make lockfile-check` and commit lockfile updates.

## Dependency Policy Triage
If `make deny` fails:

1. Advisory issue:
   - Upgrade affected crates, or
   - add a temporary justified ignore in `deny.toml` with linked tracking issue.
2. License/source issue:
   - replace dependency with compliant source/license, or
   - explicitly approve in policy with legal/security signoff.
3. Ban/wildcard issue:
   - pin explicit version in `Cargo.toml` and regenerate `Cargo.lock`.

## Incident Notes Template
For security-impacting PRs, include:

1. Trigger (what failed and where).
2. Impacted surface.
3. Mitigation implemented.
4. Residual risk.
5. Follow-up owner and due date.

# Security Policy

## Supported versions

Security fixes currently target `main`. No tagged release line exists yet; this section will identify supported release branches after the first published version.

## Reporting a vulnerability

1. Do not disclose vulnerability details in public issues or pull requests.
2. Use GitHub Security Advisories (`Security` → `Report a vulnerability`) for private disclosure.
3. If private advisories are unavailable, open a minimal issue requesting a private contact channel without technical details.

## Response targets

- Initial triage: within three business days
- Remediation plan or mitigation guidance: within seven business days for a validated report

Targets are project goals, not a paid support agreement.

## In-scope examples

- Backup, replication, scrub, or restore behavior that can publish corrupt state or destroy unrelated data
- Path traversal, symlink attacks, store-lock bypass, IPC impersonation, or unsafe service definitions
- Leakage of local paths, keys, backup contents, or unredacted diagnostics
- Dependency or build-chain vulnerabilities that affect shipped behavior

## Disclosure process

1. Reproduce and assess impact privately.
2. Prepare the fix and regression tests on a private branch.
3. Coordinate disclosure timing with the reporter when possible.
4. Publish an appropriate changelog entry when a release is available.

## Local security checks

- Combined gate: `make security-check`
- Operational guidance: [Security runbook](docs/security-runbook.md)
- System assumptions and residuals: [Threat model](docs/threat-model.md)
- Known project risks: [Risk register](docs/risk-register.md)

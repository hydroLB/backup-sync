# Security Policy

## Supported Versions
Security fixes are applied to the latest `main` branch and the latest release line.

## Reporting a Vulnerability
1. Do not disclose vulnerability details in public issues or PRs.
2. Use GitHub Security Advisories (`Security` tab, `Report a vulnerability`) for private disclosure.
3. If Security Advisories are unavailable, open a minimal issue requesting a private contact channel without technical details.

## Response Targets
- Initial triage response: within 3 business days
- Remediation plan or mitigation guidance: within 7 business days for validated reports

## Disclosure Process
1. Reproduce and validate the report.
2. Prepare a fix and tests in a private branch.
3. Coordinate release timing with reporters when possible.
4. Publish a changelog entry after patch release.

## Scope Notes
- This project handles local filesystem backup data.
- Reports about unsafe defaults, data leakage, privilege escalation, or integrity bypass are in scope.

## Security Checks and Local Guidance
- Local security runbook: [`docs/security-runbook.md`](docs/security-runbook.md)
- Threat model notes for critical surfaces: [`docs/threat-model.md`](docs/threat-model.md)
- Combined local security gate: `make security-check`

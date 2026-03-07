# Risk Register (Top 15)

Last updated: 2026-02-20  
Scope: production correctness, security, performance, availability, maintainability, and developer experience.

## Priority scale
- `P0`: high likelihood + high blast radius, should be addressed in the next delivery window.
- `P1`: significant risk that should be reduced in near-term roadmap.
- `P2`: meaningful risk, lower urgency or narrower blast radius.

## Risks and Fix Paths

| ID | Pri | Risk | Impact | Fix approach | Mapped PR IDs |
|---|---|---|---|---|---|
| R-01 | P0 | Layer violations can slip in without an automated boundary checker. | Domain logic can leak into app/UI layers, increasing coupling and regression risk. | Add import/dependency boundary checks in CI and local gate. | PR-10 |
| R-02 | P0 | Config loading and validation behavior can diverge across entrypoints. | Bad config may fail late or inconsistently across CLI/daemon/GUI. | Unify config loading/validation path with explicit fail-fast startup contract. | PR-12 |
| R-03 | P0 | Error model is not fully standardized at all process boundaries. | Operators and users receive inconsistent error shapes and recovery guidance. | Introduce typed error codes and centralized boundary mapping. | PR-13 |
| R-04 | P0 | Observability baseline is partial and not fully correlated end-to-end. | Harder incident triage and slower root-cause analysis under production pressure. | Standardize structured logs, correlation IDs, and redaction policy across boundaries. | PR-14 |
| R-05 | P0 | Coverage gates are still near minimum viable thresholds. | Subtle regressions may pass CI, especially in boundary and error paths. | Add contract-focused tests and raise thresholds incrementally with repeatability checks. | PR-17, PR-18 |
| R-06 | P1 | CI topology is mostly monolithic and provides slower/fuzzier failure isolation. | Longer feedback cycles and harder diagnosis of failures in PRs. | Split CI jobs by gate type with parallelization and artifact upload. | PR-8 |
| R-07 | P1 | Required-check policy is not codified as an explicit branch-protection contract document. | Merge policy can drift from intended safety bar. | Define and maintain required checks policy with exact check names. | PR-9 |
| R-08 | P1 | Public API surface ownership is not yet formalized for all crate boundaries. | Contract drift and accidental breaking changes across IPC/CLI/GUI surfaces. | Explicit API ownership list + minimal public exports + compatibility checks. | PR-11 |
| R-09 | P1 | Health/readiness semantics and graceful shutdown assertions need stronger test-backed guarantees. | Availability incidents can be harder to detect and recover from safely. | Implement health/readiness contracts and shutdown behavior tests/runbook hooks. | PR-15 |
| R-10 | P1 | Security automation covers scans but policy control is still incomplete. | Advisory ignores and dependency policy exceptions can drift over time. | Add policy-driven dependency/security checks and threat-model notes. | PR-16 |
| R-11 | P1 | Incident runbooks and rollback procedures are incomplete. | Longer MTTR and higher operator error during critical events. | Add focused runbooks for common failure modes and rollback steps. | PR-19 |
| R-12 | P2 | Architecture and design-decision narrative is still fragmented. | Slower onboarding and weaker long-term design consistency. | Add architecture one-pager, ADR set, and design-principles doc set. | PR-20 |
| R-13 | P2 | Release workflow still relies on partial manual process. | Inconsistent release quality and weaker auditability of shipped changes. | Automate release notes/changelog and enforce version/tag strategy. | PR-21 |
| R-14 | P2 | Perf and resilience checks focus on a narrow subset of paths. | Regressions in non-benchmarked hot paths may escape early detection. | Expand microbench/load tests and document resource tuning baselines. | PR-22 |
| R-15 | P2 | Legacy/duplicate logic pockets and global-state seams increase maintenance cost. | Harder reasoning, larger diffs, and elevated bug risk during refactors. | Execute targeted hygiene/refactor tranche with strict behavior-preserving tests. | PR-23 |

## Mapping note
Every risk in this register maps directly to at least one planned PR ID (`PR-8` through `PR-23`) so execution can be tracked with measurable closure criteria.

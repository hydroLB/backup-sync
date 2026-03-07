# Delivery Roadmap (Post PR-7)

Last updated: 2026-02-20

This roadmap starts after completion of PR-1 through PR-7.

## Phase 1: Baseline quality gates and safety

### Goal
Harden deterministic quality gates, security policy baseline, and test reliability so day-to-day development is safe by default.

### Planned PRs
- PR-8: CI topology hardening
- PR-9: Branch protection and required-check contract
- PR-16: Security baseline completion
- PR-17: Test determinism and contract coverage
- PR-18: Coverage gate tightening (incremental)

### Measurable Definition of Done
1. CI has discrete required jobs for format, lint, typecheck, tests, coverage, build, and security.
2. Branch protection required checks are documented and match actual CI job names.
3. Security policy checks are automated with explicit exception governance.
4. Test suite is reproducible across repeated local and CI runs.
5. Coverage thresholds are higher than current baseline and pass in repeated runs.

## Phase 2: Architecture hardening and operability

### Goal
Enforce clean module boundaries, stabilize API/error/config contracts, and improve production operability signals.

### Planned PRs
- PR-10: Layer dependency enforcement
- PR-11: Public API surface hardening
- PR-12: Config unification and fail-fast validation
- PR-13: Unified error model
- PR-14: Observability baseline
- PR-15: Operability primitives
- PR-19: Runbooks and rollback playbooks

### Measurable Definition of Done
1. Automated boundary checks fail CI on forbidden dependency direction.
2. Public API surfaces are explicit and documented with ownership.
3. All process entrypoints use a single validated config startup path.
4. Error responses are structured and consistent for CLI, daemon, and GUI boundaries.
5. Structured logs include correlation IDs and redaction-safe fields.
6. Health/readiness and graceful shutdown behavior are test-backed.
7. Runbooks exist for top operational incidents with rollback steps.

## Phase 3: Scalability, polish, and long-term maintainability

### Goal
Complete recruiter-grade architecture narrative, release discipline, and performance/resilience expansion while reducing long-term codebase drag.

### Planned PRs
- PR-20: Architecture narrative, ADRs, and design principles
- PR-21: Release engineering discipline
- PR-22: Performance and resilience polish
- PR-23: Hygiene/refactor tranche

### Measurable Definition of Done
1. Architecture doc + ADR set + design principles are complete and internally consistent.
2. Release process is automated and repeatable with policy-backed versioning.
3. Hot-path benchmarks and selected load/resilience checks are CI-enforced.
4. Dead/duplicated code and global-state seams are reduced with behavior-preserving tests.
5. New contributors can locate ownership, boundaries, and runbooks within 10 minutes from README.

## Risk closure mapping
- Phase 1 primarily closes: `R-05`, `R-06`, `R-07`, `R-10`
- Phase 2 primarily closes: `R-01`, `R-02`, `R-03`, `R-04`, `R-08`, `R-09`, `R-11`
- Phase 3 primarily closes: `R-12`, `R-13`, `R-14`, `R-15`

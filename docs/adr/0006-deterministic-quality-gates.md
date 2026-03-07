# ADR 0006: Deterministic Quality Gates as Merge Contract

- Status: Accepted
- Date: 2026-02-20

## Context
Inconsistent local environments and ad hoc CI checks produce false failures and make regressions harder to catch before merge.

## Decision
Make deterministic quality gates a hard merge contract, with `make check` and `make ci` as canonical entrypoints and pinned toolchains/runtime versions.

## Consequences
- Fresh clones can run one command for reproducible quality verification.
- Local and CI gate sequences remain aligned.
- Recruiters and maintainers can validate project quality quickly.
- Adding a new gate requires updating one canonical contract instead of many scripts.

## Alternatives considered
- Decentralized per-language scripts only (rejected for drift).
- Best-effort CI without required checks (rejected for reliability risk).

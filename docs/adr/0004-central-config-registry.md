# ADR 0004: Centralized Config Registry with Fail-Fast Validation

- Status: Accepted
- Date: 2026-02-20

## Context
Config values are consumed by multiple processes and hot paths. Distributed defaults and weak validation create startup drift and runtime surprises.

## Decision
Maintain one config registry for defaults and constraints, with environment layering and startup validation that fails fast on invalid inputs.

## Consequences
- Entrypoints converge on one loading path with consistent behavior.
- Invalid configuration fails early with actionable error output.
- Tunable knobs are discoverable and documented from one source.
- Runtime branches for malformed config are reduced.

## Alternatives considered
- Lazy per-module config parsing (rejected for inconsistency and delayed failures).
- Dynamic remote config service (rejected as unnecessary for local-first deployment).

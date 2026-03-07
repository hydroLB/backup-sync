# ADR 0005: Unified Boundary Error Model and Structured Logging

- Status: Accepted
- Date: 2026-02-20

## Context
Different error formats across CLI, daemon, and GUI make incidents harder to triage and reduce confidence in automated diagnostics.

## Decision
Adopt typed boundary errors and centralized mapping, paired with structured logs using stable fields and correlation IDs.

## Consequences
- Operators receive consistent, actionable failure outputs across interfaces.
- Logs become machine-parseable for incident workflows and automated analysis.
- Silent catches are removed from boundary paths.
- Test coverage can assert error codes and mapping behavior.

## Alternatives considered
- Free-form error strings only (rejected for low operability).
- Per-interface logging conventions (rejected due to analysis friction).

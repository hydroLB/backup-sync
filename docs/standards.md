# Engineering Standards

This project follows official language and ecosystem standards and applies them consistently to every source file.

## Rust
- Rust Style Guide via rustfmt
- Rust API Guidelines
- Rustdoc comment conventions
- Clippy linting policy with warnings treated as errors

## TypeScript and React
- TypeScript Handbook language rules and strict mode expectations
- React official documentation for component, hook, and error boundary patterns
- TSDoc conventions for function and method documentation

## Documentation format
- Public APIs document their contract, important failure modes, and any non-obvious side effects.
- Internal comments explain invariants, safety boundaries, tradeoffs, or decisions that code alone cannot make clear.
- Do not narrate signatures, repeat method names, or add template headings to self-explanatory functions and tests.
- Test names should describe the protected behavior; comments are reserved for unusual fixture constraints or failure scenarios.
- The repository hygiene gate rejects legacy `Summary`/`Inputs`/`Outputs` comment templates.
- Documentation links and anchors must pass `make docs-check`.
- Comments are ASCII unless a user-facing string or external contract requires otherwise.

## Error handling
- Fallible operations return explicit errors with method names and context.
- All error messages identify the exact method where the failure occurred and the operation that triggered it.

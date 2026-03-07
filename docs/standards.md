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
- Every function or method uses the same doc comment headings in this order: Summary, Inputs, Outputs, Side effects, Error handling, Ties to other methods, Why this exists.
- Each heading is a single sentence or short clause describing the method in that section.
- Comments are ASCII only and avoid em dashes.

## Error handling
- Fallible operations return explicit errors with method names and context.
- All error messages identify the exact method where the failure occurred and the operation that triggered it.

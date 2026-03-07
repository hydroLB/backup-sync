# Contributing

## Scope
This repository accepts focused, production-ready changes. Keep pull requests small, behavior-preserving by default, and easy to review.

## Development Setup
1. Install pinned toolchains:
   - Node from `.nvmrc`
   - Rust from `rust-toolchain.toml`
2. Install frontend dependencies:
   - `make frontend-install`
3. Enable local hooks:
   - `make hooks`

## Command Contract
- Full quality gate: `make check`
- CI alias: `make ci`
- Local app run: `make dev`
- Optional formatting pass: `make fmt` and `make frontend-format`

## Change Rules
1. Preserve behavior unless the current behavior is clearly incorrect.
2. Add or update tests before changing behavior.
3. Keep diffs scoped to one concern.
4. Avoid adding dependencies unless there is a clear need.
5. Use deterministic tests and fixtures only.

## Pull Request Process
1. Create a branch from `main`.
2. Implement the change with tests and docs updates as needed.
3. Run `make check` locally.
4. Open a PR using the default template.
5. Ensure required checks pass before requesting merge.

## Commit Guidance
- Use concise, imperative commit subjects.
- Group related changes in the same commit.
- Do not mix formatting-only changes with behavior changes unless required by tooling.

## Reporting Security Issues
Do not open public issues for vulnerabilities. Follow `/SECURITY.md`.

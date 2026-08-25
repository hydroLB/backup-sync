# Contributing

## Scope

Backup Sync accepts focused, production-ready changes. Correctness and recovery behavior take priority over convenience, especially around store metadata, path handling, concurrency, and target replacement.

## Development setup

1. Install Rust 1.93.0 from `rust-toolchain.toml`, Node 22.22.0 from `.nvmrc`, Python 3, npm, Make, and the host's Tauri 2 prerequisites.
2. Run `make frontend-install`.
3. Run `make hooks` to enable repository hooks.
4. Launch with `./start`, or read [Getting started](docs/getting-started.md).

## Command contract

- Full local/CI gate: `make check` (`make ci` is an alias)
- Backend tests: `make test`
- Frontend tests: `make frontend-test`
- Documentation validation: `make docs-check`
- Security bundle: `make security-check`
- Performance guards: `make perf-check`
- Development app: `./start` or `make dev`

The full gate installs pinned audit/coverage tools, scans git history, and may require network access. Run focused checks while iterating, then the full contract before review.

## Change rules

1. Add a regression test for behavior changes, especially destructive or cross-process paths.
2. Keep domain and local-storage rules in `crates/core`; keep daemon, CLI, GUI, and frontend adapters thin.
3. Treat manifests, indexes, paths, IPC, environment values, and diagnostic filenames as untrusted input.
4. Preserve crash ordering: durable content before manifest, manifest before index, verified staging before target replacement.
5. Avoid new dependencies unless the need and supply-chain impact are clear.
6. Update the relevant contract doc, risk, ADR, or runbook when behavior changes.
7. Do not claim platform runtime support based only on conditional compilation.

## Pull requests

1. Branch from `main`.
2. Keep the diff scoped and explain the failure mode or invariant it addresses.
3. Include tests and documentation in the same change.
4. Run `make check` and report any environment-specific validation that could not run.
5. Ensure required GitHub checks pass and review conversations are resolved.

Use concise, imperative commit subjects. Keep formatting-only work separate unless tooling requires it.

## Security reports

Do not disclose vulnerabilities in public issues or pull requests. Follow [SECURITY.md](SECURITY.md).

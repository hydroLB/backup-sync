# Getting Started

Last updated: 2026-08-23

Backup Sync is currently built and run from source. These steps launch the development desktop app; they do not install a packaged system application.

## Prerequisites

- Rust 1.93.0, pinned in `rust-toolchain.toml`
- Node.js 22.22.0, pinned in `.nvmrc`
- npm and Python 3
- Git and GNU Make
- The [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for the host OS

Use the pinned versions when reproducing CI failures. The Linux CI setup script at `scripts/ci/install-tauri-linux-deps.sh` is the repository's executable reference for native packages.

## Build and launch on macOS or Linux

```bash
git clone https://github.com/hydroLB/backup-sync.git
cd backup-sync
make frontend-install
./start
```

The launcher verifies prerequisites, fetches locked Rust dependencies, builds the daemon, installs locked frontend dependencies, and starts the Tauri development app. It starts the daemon after a config file becomes available. Vite listens on fixed port `5173`; stop a conflicting process instead of changing the port.

`./start` is a Bash launcher for macOS and Linux. Windows workspace compilation runs in CI, but the project does not yet claim a Windows-native first-run workflow, named-pipe runtime validation, or service-install smoke test. On Windows, `cargo build --workspace` and `npm --prefix crates/gui/frontend run build` are compile checks only; use a disposable environment if you evaluate beyond them.

To build without launching:

```bash
make build
```

This compiles the Rust workspace into `target/debug` and builds frontend assets into `crates/gui/frontend/dist`. It does not create a native installer.

## First configuration

The desktop app can create the basic source and destination mapping. For a terminal-guided setup, use:

```bash
cargo run -p cli -- init
```

Review the result before the first write:

```bash
cargo run -p cli -- run-once --dry-run
cargo run -p cli -- doctor
```

Then run a backup and verify the store:

```bash
cargo run -p cli -- run-once
cargo run -p cli -- verify
```

The configuration reference documents [paths, environment overrides, destinations, replication, and advanced settings](configuration.md).

## Background service

Install and enable the per-user service only after a successful manual backup and verification:

```bash
cargo run -p cli -- install-service --enable
cargo run -p cli -- status
```

The generated service definition uses the current executable and config path. Service behavior is platform-specific; this source repository does not yet claim clean-machine runtime validation on macOS or Windows.

## Desktop behavior

- Closing the window hides it without starting a backup or changing configuration.
- Quitting closes the GUI; an installed daemon service continues running.
- The Running toggle disables or enables background writes through safe mode.
- Advanced schedule and storage options are edited in `config.toml`.

## Development commands

```bash
make frontend-test
make test
make docs-check
make perf-check
make check
```

`make check` is the canonical full gate and includes builds, backend tests, coverage, performance, secret scanning, dependency policy, and audits. Some targets install pinned Cargo tools and require network access.

## Safe evaluation

Use disposable source and destination directories when evaluating behavior. Never point both at overlapping trees, and do not enable encryption until the generated key has a tested recovery copy. A backup is not proven until a representative restore has been opened and checked.

For incidents, start with the [runbook index](runbooks/README.md). For known limitations, see the [risk register](risk-register.md).

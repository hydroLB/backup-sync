fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

hooks:
	git config core.hooksPath .githooks

frontend-install:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm ci

frontend-lint:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm run lint

frontend-format:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm run format

frontend-format-check:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm run format:check

frontend-typecheck:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm run typecheck

frontend-test:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm test

frontend-coverage:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm run test:coverage

test:
	cargo test -p backup_core -p daemon -p cli

build:
	cargo build --workspace
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm ci && npm_config_cache=$(CURDIR)/.npm-cache npm run build

run:
	./launch.sh

bench:
	cargo bench -p backup_core --features bench

perf-record:
	./scripts/perf/record_baseline.sh

perf-check:
	./scripts/perf/check_baseline.sh

coverage:
	rustup component add llvm-tools-preview
	cargo install cargo-llvm-cov --locked
	cargo llvm-cov -p backup_core -p daemon -p cli --fail-under-lines 60 --fail-under-functions 60 --fail-under-branches 50
	$(MAKE) frontend-coverage

audit:
	cargo install cargo-audit --locked
	cargo audit --deny warnings
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm audit --audit-level=high --omit=dev

ci:
	$(MAKE) fmt-check
	$(MAKE) clippy
	$(MAKE) frontend-lint
	$(MAKE) frontend-format-check
	$(MAKE) frontend-typecheck
	$(MAKE) perf-check
	$(MAKE) coverage
	$(MAKE) audit

dev:
	./launch.sh

.PHONY: fmt fmt-check clippy hooks frontend-install frontend-lint frontend-format frontend-format-check frontend-typecheck frontend-test frontend-coverage test build run bench perf-record perf-check coverage audit ci dev

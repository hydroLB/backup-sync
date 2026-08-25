fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

rustdoc-check:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

hooks:
	git config core.hooksPath .githooks

clean-local:
	./scripts/clean-local-artifacts.sh

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

# test:coverage executes the complete frontend test suite and enforces coverage.
# The canonical `check` graph uses this target instead of re-running frontend-test.
frontend-coverage:
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm run test:coverage

secrets-working-tree:
	mkdir -p .reports/security
	gitleaks dir . --redact --report-format sarif --report-path .reports/security/gitleaks-working-tree.sarif

secrets-history:
	mkdir -p .reports/security
	gitleaks git --redact --log-opts="--all" --report-format sarif --report-path .reports/security/gitleaks-history.sarif

secrets:
	$(MAKE) secrets-working-tree
	$(MAKE) secrets-history

lockfile-check:
	./scripts/check-lockfile-hygiene.sh

deny:
	cargo install cargo-deny --locked --version 0.19.4 --debug
	cargo deny check --config deny.toml --hide-inclusion-graph advisories bans licenses sources

security-check:
	$(MAKE) secrets
	$(MAKE) lockfile-check
	$(MAKE) deny
	$(MAKE) audit

hygiene-check:
	@if git ls-files | rg -n "(^|/)\\.DS_Store$$|Thumbs\\.db$$|Desktop\\.ini$$" >/dev/null; then \
		echo "Tracked OS junk files detected. Remove them from git."; \
		git ls-files | rg "(^|/)\\.DS_Store$$|Thumbs\\.db$$|Desktop\\.ini$$"; \
		exit 1; \
	fi
	./scripts/check-repo-hygiene.sh
	./scripts/tests/check-repo-hygiene-smoke.sh

boundaries-check:
	./scripts/check-boundaries.sh

operability-check:
	./scripts/check-operability.sh

docs-check:
	python3 ./scripts/check-docs.py
	./scripts/tests/check-docs-smoke.sh

desktopctl-test:
	PYTHONPATH=tools/desktopctl python3 -m unittest discover -s tools/desktopctl/tests -p 'test_*.py'

test:
	$(MAKE) test-unit
	$(MAKE) test-integration
	$(MAKE) test-e2e

test-unit:
	cargo test -p backup_core --lib
	cargo test -p daemon --lib
	cargo test -p cli --bins
	cargo test -p gui --lib --bins
	cargo test -p gui-app --bins

test-integration:
	@set -e; \
	for test_file in $$(find crates/core/tests -name '*.rs' ! -name 'e2e_smoke.rs' ! -path '*/support/*' | sort); do \
		test_name=$${test_file##*/}; \
		test_name=$${test_name%.rs}; \
		cargo test -p backup_core --test "$$test_name"; \
	done; \
	for test_file in $$(find crates/daemon/tests -name '*.rs' ! -path '*/support/*' | sort); do \
		test_name=$${test_file##*/}; \
		test_name=$${test_name%.rs}; \
		cargo test -p daemon --test "$$test_name"; \
	done; \
	for test_file in $$(find crates/cli/tests -name '*.rs' ! -path '*/support/*' | sort); do \
		test_name=$${test_file##*/}; \
		test_name=$${test_name%.rs}; \
		cargo test -p cli --test "$$test_name"; \
	done; \
	for test_file in $$(find crates/gui/tests -name '*.rs' ! -path '*/support/*' | sort); do \
		test_name=$${test_file##*/}; \
		test_name=$${test_name%.rs}; \
		cargo test -p gui --test "$$test_name"; \
	done

test-e2e:
	cargo test -p backup_core --test e2e_smoke

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

perf-ipc-load:
	cargo test -p daemon --test ipc_load -- --nocapture

coverage:
	rustup component add llvm-tools-preview
	cargo install cargo-llvm-cov --locked --version 0.8.5
	cargo llvm-cov -p backup_core -p daemon -p cli --fail-under-lines 53 --fail-under-functions 48 --fail-under-regions 53
	$(MAKE) frontend-coverage

audit:
	cargo install cargo-audit --locked --version 0.22.1
	./scripts/security/run_cargo_audit.sh
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm audit --audit-level=high

release-validate:
	@if [ -z "$(VERSION)" ]; then \
		echo "Usage: make release-validate VERSION=X.Y.Z"; \
		exit 1; \
	fi
	./scripts/release/validate_release_ref.sh "HEAD" "origin/main"
	./scripts/release/validate_changelog_for_release.sh "$(VERSION)" CHANGELOG.md
	./scripts/release/validate_semver_tag.sh "v$(VERSION)"

check:
	$(MAKE) fmt-check
	$(MAKE) clippy
	$(MAKE) rustdoc-check
	$(MAKE) boundaries-check
	$(MAKE) operability-check
	$(MAKE) frontend-lint
	$(MAKE) frontend-format-check
	$(MAKE) frontend-typecheck
	$(MAKE) hygiene-check
	$(MAKE) docs-check
	$(MAKE) desktopctl-test
	$(MAKE) lockfile-check
	$(MAKE) perf-check
	$(MAKE) test
	$(MAKE) build
	$(MAKE) coverage
	$(MAKE) security-check

ci: check

dev:
	./launch.sh

.PHONY: fmt fmt-check clippy rustdoc-check hooks clean-local frontend-install frontend-lint frontend-format frontend-format-check frontend-typecheck frontend-test frontend-coverage secrets-working-tree secrets-history secrets lockfile-check deny security-check hygiene-check boundaries-check operability-check docs-check desktopctl-test test test-unit test-integration test-e2e build run bench perf-record perf-check perf-ipc-load coverage audit release-validate check ci dev

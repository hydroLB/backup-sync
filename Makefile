fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

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
	cargo install cargo-deny --locked --debug
	cargo deny check --config deny.toml advisories bans licenses sources

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

boundaries-check:
	./scripts/check-boundaries.sh

operability-check:
	./scripts/check-operability.sh

test:
	$(MAKE) test-unit
	$(MAKE) test-integration
	$(MAKE) test-e2e

test-unit:
	cargo test -p backup_core --lib
	cargo test -p daemon --lib
	cargo test -p cli --bins

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
	cargo install cargo-llvm-cov --locked
	cargo llvm-cov -p backup_core -p daemon -p cli --fail-under-lines 53 --fail-under-functions 48 --fail-under-regions 53
	$(MAKE) frontend-coverage

audit:
	cargo install cargo-audit --locked
	cargo audit --deny warnings \
		--ignore RUSTSEC-2024-0413 \
		--ignore RUSTSEC-2024-0416 \
		--ignore RUSTSEC-2025-0057 \
		--ignore RUSTSEC-2024-0412 \
		--ignore RUSTSEC-2024-0418 \
		--ignore RUSTSEC-2024-0411 \
		--ignore RUSTSEC-2024-0417 \
		--ignore RUSTSEC-2024-0414 \
		--ignore RUSTSEC-2024-0415 \
		--ignore RUSTSEC-2024-0420 \
		--ignore RUSTSEC-2024-0419 \
		--ignore RUSTSEC-2024-0384 \
		--ignore RUSTSEC-2024-0370 \
		--ignore RUSTSEC-2024-0429 \
		--ignore RUSTSEC-2025-0075 \
		--ignore RUSTSEC-2025-0080 \
		--ignore RUSTSEC-2025-0081 \
		--ignore RUSTSEC-2025-0098 \
		--ignore RUSTSEC-2025-0100
	cd crates/gui/frontend && npm_config_cache=$(CURDIR)/.npm-cache npm audit --audit-level=high --omit=dev

release-validate:
	@if [ -z "$(VERSION)" ]; then \
		echo "Usage: make release-validate VERSION=X.Y.Z"; \
		exit 1; \
	fi
	./scripts/release/validate_changelog_for_release.sh "$(VERSION)" CHANGELOG.md
	./scripts/release/validate_semver_tag.sh "v$(VERSION)"

check:
	$(MAKE) fmt-check
	$(MAKE) clippy
	$(MAKE) boundaries-check
	$(MAKE) operability-check
	$(MAKE) frontend-lint
	$(MAKE) frontend-format-check
	$(MAKE) frontend-typecheck
	$(MAKE) hygiene-check
	$(MAKE) lockfile-check
	$(MAKE) perf-check
	$(MAKE) coverage
	$(MAKE) security-check

ci: check

dev:
	./launch.sh

.PHONY: fmt fmt-check clippy hooks clean-local frontend-install frontend-lint frontend-format frontend-format-check frontend-typecheck frontend-test frontend-coverage secrets-working-tree secrets-history secrets lockfile-check deny security-check hygiene-check boundaries-check operability-check test test-unit test-integration test-e2e build run bench perf-record perf-check perf-ipc-load coverage audit release-validate check ci dev

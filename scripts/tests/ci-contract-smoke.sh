#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_path="${1:-${repo_root}/.reports/ci/ci-contract.txt}"

mkdir -p "$(dirname "${output_path}")"
make -C "${repo_root}" -n ci >"${output_path}"

require_in_dry_run() {
  local expected="$1"
  if ! grep -Fq -- "${expected}" "${output_path}"; then
    echo "[ci-contract] canonical make ci graph is missing: ${expected}" >&2
    exit 1
  fi
}

# Static, documentation, performance, coverage, and security gates.
for expected in \
  'cargo fmt --all -- --check' \
  'cargo clippy --all-targets --all-features -- -D warnings' \
  'cargo doc --workspace --all-features --no-deps' \
  './scripts/check-boundaries.sh' \
  './scripts/check-operability.sh' \
  'npm run lint' \
  'npm run format:check' \
  'npm run typecheck' \
  './scripts/check-repo-hygiene.sh' \
  'python3 ./scripts/check-docs.py' \
  "PYTHONPATH=tools/desktopctl python3 -m unittest discover -s tools/desktopctl/tests -p 'test_*.py'" \
  './scripts/check-lockfile-hygiene.sh' \
  './scripts/perf/check_baseline.sh' \
  'cargo llvm-cov' \
  'npm run test:coverage' \
  'gitleaks dir .' \
  'gitleaks git ' \
  'cargo deny check' \
  './scripts/security/run_cargo_audit.sh' \
  'npm audit --audit-level=high'
do
  require_in_dry_run "${expected}"
done

# Complete Rust test/build coverage, including the Tauri workspace members.
for expected in \
  'cargo test -p backup_core --lib' \
  'cargo test -p daemon --lib' \
  'cargo test -p cli --bins' \
  'cargo test -p gui --lib --bins' \
  'cargo test -p gui-app --bins' \
  'crates/gui/tests' \
  'cargo test -p backup_core --test e2e_smoke' \
  'cargo build --workspace' \
  'npm run build'
do
  require_in_dry_run "${expected}"
done

if grep -Fq -- '--omit=dev' "${output_path}"; then
  echo '[ci-contract] npm audit must include frontend dev/build dependencies' >&2
  exit 1
fi

# Hosted CI must compile native cfg branches on both supported desktop runners.
for expected in \
  'os: macos-14' \
  'os: windows-2022' \
  'cargo check --workspace --all-targets --all-features --locked'
do
  if ! grep -Fq -- "${expected}" "${repo_root}/.github/workflows/ci.yml"; then
    echo "[ci-contract] hosted platform compile contract is missing: ${expected}" >&2
    exit 1
  fi
done

# CodeQL's Rust build must install Tauri prerequisites and build every member.
for expected in \
  './scripts/ci/install-tauri-linux-deps.sh' \
  'cargo build --workspace --all-features --locked'
do
  if ! grep -Fq -- "${expected}" "${repo_root}/.github/workflows/codeql.yml"; then
    echo "[ci-contract] CodeQL build contract is missing: ${expected}" >&2
    exit 1
  fi
done

echo '[ci-contract] canonical local and hosted gate contracts passed'

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[lockfile] required command not found: $cmd"
    exit 1
  fi
}

require_file() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "[lockfile] required lockfile missing: $file"
    exit 1
  fi
}

require_cmd cargo
require_cmd npm
require_cmd shasum

require_file Cargo.lock
require_file crates/gui/frontend/package-lock.json

hash_file() {
  local file="$1"
  shasum -a 256 "$file" | awk '{print $1}'
}

before_cargo_hash="$(hash_file Cargo.lock)"
before_npm_hash="$(hash_file crates/gui/frontend/package-lock.json)"

# Cargo.lock must resolve fully in locked mode.
cargo metadata --format-version 1 --locked --no-deps >/dev/null

# package-lock.json must be installable with npm ci without mutating lock state.
(
  cd crates/gui/frontend
  npm_config_cache="$ROOT_DIR/.npm-cache" npm ci --ignore-scripts --dry-run >/dev/null
)

after_cargo_hash="$(hash_file Cargo.lock)"
after_npm_hash="$(hash_file crates/gui/frontend/package-lock.json)"

# In CI and local checks, lockfiles should not be rewritten by hygiene checks.
if [[ "$before_cargo_hash" != "$after_cargo_hash" || "$before_npm_hash" != "$after_npm_hash" ]]; then
  echo "[lockfile] lockfiles changed during hygiene check; regenerate and commit them"
  exit 1
fi

echo "[lockfile] OK: Cargo.lock and package-lock.json are present and consistent"

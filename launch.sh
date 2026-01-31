#!/usr/bin/env bash
set -euo pipefail

# Bulletproof Tauri launcher: verifies prereqs, installs deps, and runs the app with clear errors.

ROOT="$(cd "$(dirname "$0")" && pwd)"
GUI_ROOT="$ROOT/crates/gui"
FRONTEND="$GUI_ROOT/frontend"
TAURI_CONF="$GUI_ROOT/src-tauri/tauri.conf.json"

error() { echo "Error: $1" >&2; exit 1; }

command -v npm >/dev/null 2>&1 || error "npm is not installed or not on PATH."
[ -d "$GUI_ROOT" ] || error "Expected gui folder at $GUI_ROOT"
[ -f "$FRONTEND/package.json" ] || error "Missing frontend/package.json (are you in the repo root?)."
[ -f "$TAURI_CONF" ] || error "Missing Tauri config at $TAURI_CONF"

cd "$GUI_ROOT"

if [ ! -d "$FRONTEND/node_modules" ]; then
  echo "Installing frontend dependencies..."
  npm --prefix "$FRONTEND" ci || error "npm ci failed in $FRONTEND"
fi

# Ensure Tauri CLI binary exists (installed in frontend devDependencies)
TAURI_BIN="$FRONTEND/node_modules/.bin/tauri"
[ -x "$TAURI_BIN" ] || error "Tauri CLI not found at $TAURI_BIN. Run npm install again."

echo "Starting Tauri app..."
# Run Tauri from the gui crate (so Cargo.toml/src-tauri are discoverable)
cd "$GUI_ROOT"
"$TAURI_BIN" dev --config src-tauri/tauri.conf.json || error "Tauri failed to start"

#!/usr/bin/env bash
set -euo pipefail

# Bulletproof Tauri launcher: verifies prereqs, installs deps, and runs the app with clear errors.

ROOT="$(cd "$(dirname "$0")" && pwd)"
GUI_ROOT="$ROOT/crates/gui"
FRONTEND="$GUI_ROOT/frontend"
TAURI_CONF="$GUI_ROOT/tauri.conf.json"
NPM_CACHE_DIR="$ROOT/.npm-cache"
DEFAULT_TAURI_DEV_PORT="5173"
DAEMON_PID=""

error() { echo "Error: $1" >&2; exit 1; }

assert_live_mode_config() {
  local conf_path="$1"
  rg -q '"beforeDevCommand"[[:space:]]*:[[:space:]]*"cd \.\./frontend && npm run dev"' "$conf_path" \
    || error "Live mode requires beforeDevCommand to run the Vite dev server in $conf_path"
  rg -q '"devPath"[[:space:]]*:[[:space:]]*"http://localhost:5173"' "$conf_path" \
    || error "Live mode requires devPath to point at http://localhost:5173 in $conf_path"
}

command -v npm >/dev/null 2>&1 || error "npm is not installed or not on PATH."
command -v cargo >/dev/null 2>&1 || error "cargo is not installed or not on PATH."
command -v rustc >/dev/null 2>&1 || error "rustc is not installed or not on PATH."
command -v python3 >/dev/null 2>&1 || error "python3 is not installed or not on PATH."
[ -d "$GUI_ROOT" ] || error "Expected gui folder at $GUI_ROOT"
[ -f "$FRONTEND/package.json" ] || error "Missing frontend/package.json (are you in the repo root?)."
[ -f "$TAURI_CONF" ] || error "Missing Tauri config at $TAURI_CONF"

if [ "${BACKUP_SYNC_FORCE_LIVE_MODE:-0}" = "1" ]; then
  assert_live_mode_config "$TAURI_CONF"
fi

# Use a repo-local npm cache so dev workflows are resilient to global cache permission issues.
mkdir -p "$NPM_CACHE_DIR" || error "Failed to create npm cache directory at $NPM_CACHE_DIR"
export npm_config_cache="$NPM_CACHE_DIR"

echo "Checking and downloading Rust dependencies..."
if [ -f "$ROOT/Cargo.lock" ]; then
  cargo fetch --locked --manifest-path "$ROOT/Cargo.toml" || error "cargo fetch --locked failed. Ensure Cargo.lock is up to date and dependencies are reachable."
else
  cargo fetch --manifest-path "$ROOT/Cargo.toml" || error "cargo fetch failed. Ensure dependencies are reachable."
fi

echo "Building daemon (target/debug/daemon)..."
cargo build -p daemon || error "Failed to build daemon. Ensure Rust is installed and the workspace builds: cargo build -p daemon"
DAEMON_BIN="$ROOT/target/debug/daemon"
[ -x "$DAEMON_BIN" ] || error "Daemon binary missing after build at $DAEMON_BIN"

config_dir() {
  if [ -n "${XDG_CONFIG_HOME:-}" ]; then
    echo "$XDG_CONFIG_HOME"
    return 0
  fi
  case "$(uname -s)" in
    Darwin) echo "$HOME/Library/Application Support" ;;
    *) echo "$HOME/.config" ;;
  esac
}

data_dir() {
  if [ -n "${XDG_DATA_HOME:-}" ]; then
    echo "$XDG_DATA_HOME"
    return 0
  fi
  case "$(uname -s)" in
    Darwin) echo "$HOME/Library/Application Support" ;;
    *) echo "$HOME/.local/share" ;;
  esac
}

ipc_socket_path() {
  local runtime="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}"
  echo "${runtime%/}/backup_sync_ipc.sock"
}

daemon_reachable() {
  local sock
  sock="$(ipc_socket_path)"
  python3 - "$sock" <<'PY'
import json
import socket
import sys

sock_path = sys.argv[1]
try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(0.2)
    s.connect(sock_path)
    s.sendall(b'{"type":"Status"}')
    # Not strictly required anymore, but keeps compatibility with older daemon builds.
    try:
        s.shutdown(socket.SHUT_WR)
    except OSError:
        pass
    _ = s.recv(4096)
    sys.exit(0)
except Exception:
    sys.exit(1)
PY
}

start_daemon_in_background() {
  if [ "${BACKUP_SYNC_AUTO_START_DAEMON:-1}" = "0" ]; then
    return 0
  fi

  if daemon_reachable; then
    return 0
  fi

  local cfg
  cfg="$(config_dir)/backup_sync/config.toml"
  local log
  log="$(data_dir)/backup_sync/logs/daemon.log"

  echo "Waiting for config at $cfg to start daemon..."
  local waited=0
  while [ ! -f "$cfg" ]; do
    sleep 1
    waited=$((waited + 1))
    if [ "$waited" -ge 600 ]; then
      echo "Config not found after 10 minutes; skipping daemon autostart."
      return 0
    fi
  done

  if daemon_reachable; then
    return 0
  fi

  mkdir -p "$(dirname "$log")" || true

  echo "Starting daemon in background..."
  BACKUP_SYNC_CONFIG="$cfg" BACKUP_SYNC_LOG="$log" "$DAEMON_BIN" >/dev/null 2>&1 &
  DAEMON_PID="$!"
}

cleanup() {
  if [ -n "${DAEMON_PID:-}" ] && kill -0 "$DAEMON_PID" >/dev/null 2>&1; then
    kill "$DAEMON_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

start_daemon_in_background &

cd "$GUI_ROOT"

echo "Checking and downloading frontend dependencies..."
npm --prefix "$FRONTEND" ci || error "npm ci failed in $FRONTEND"

# Ensure Tauri CLI binary exists (installed in frontend devDependencies)
TAURI_BIN="$FRONTEND/node_modules/.bin/tauri"
[ -x "$TAURI_BIN" ] || error "Tauri CLI not found at $TAURI_BIN. Run npm install again."

echo "Starting Tauri app..."
# Run Tauri from the gui crate (so Cargo.toml/src-tauri are discoverable)
cd "$GUI_ROOT"

# Keep Tauri devPath and Vite dev server port aligned by default.
if [ "${BACKUP_SYNC_FORCE_LIVE_MODE:-0}" = "1" ]; then
  export BACKUP_SYNC_DEV_PORT="$DEFAULT_TAURI_DEV_PORT"
else
  export BACKUP_SYNC_DEV_PORT="${BACKUP_SYNC_DEV_PORT:-$DEFAULT_TAURI_DEV_PORT}"
fi

"$TAURI_BIN" dev --config "$TAURI_CONF" || error "Tauri failed to start"

#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

python3 - "$ROOT" <<'PY'
import json
import os
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
canonical = root / "crates/gui/tauri.conf.json"
frontend_link = root / "crates/gui/frontend/tauri.conf.json"
obsolete = root / "crates/gui/src-tauri/tauri.conf.json"

config = json.loads(canonical.read_text(encoding="utf-8"))
build = config["build"]
assert build["devUrl"] == "http://localhost:5173"
assert build["frontendDist"] == "frontend/dist"
assert build["beforeDevCommand"] == "npm run dev"
assert build["beforeBuildCommand"] == "npm run build"
assert config["identifier"] == "com.hydrolb.backupsync"

security = config["app"]["security"]
production_csp = security["csp"]
development_csp = security["devCsp"]
assert security["freezePrototype"] is True
assert security.get("dangerousDisableAssetCspModification", False) is False
assert production_csp == {
    "default-src": ["'self'"],
    "script-src": ["'self'"],
    "style-src": ["'self'"],
    "connect-src": ["'self'", "ipc:", "http://ipc.localhost"],
    "img-src": ["'self'", "asset:", "http://asset.localhost", "data:", "blob:"],
    "font-src": ["'self'", "data:"],
    "object-src": ["'none'"],
    "base-uri": ["'self'"],
    "form-action": ["'self'"],
    "frame-ancestors": ["'none'"],
}
assert development_csp["script-src"] == [
    "'self'",
    "'unsafe-inline'",
    "http://localhost:5173",
]
assert development_csp["style-src"] == [
    "'self'",
    "'unsafe-inline'",
    "http://localhost:5173",
]
assert development_csp["connect-src"] == [
    "'self'",
    "ipc:",
    "http://ipc.localhost",
    "http://localhost:5173",
    "ws://localhost:5173",
]
assert "'unsafe-eval'" not in development_csp["script-src"]
assert "'unsafe-inline'" not in production_csp["script-src"]
assert "'unsafe-inline'" not in production_csp["style-src"]
assert "http:" not in production_csp["connect-src"]
assert "https:" not in production_csp["connect-src"]

assert frontend_link.is_symlink()
assert os.path.samefile(frontend_link, canonical)
assert not obsolete.exists()

vite = (root / "crates/gui/frontend/vite.config.ts").read_text(encoding="utf-8")
assert "port: 5173" in vite
assert "strictPort: true" in vite
assert "BACKUP_SYNC_DEV_PORT" not in vite
assert "VITE_PORT" not in vite

package = json.loads(
    (root / "crates/gui/frontend/package.json").read_text(encoding="utf-8")
)
assert package["scripts"]["tauri"] == "cd .. && tauri dev --config tauri.conf.json"

gui_manifest = (root / "crates/gui/Cargo.toml").read_text(encoding="utf-8")
assert 'default = ["single-instance"]' in gui_manifest

launcher = (root / "launch.sh").read_text(encoding="utf-8")
assert 'TAURI_CONF="$GUI_ROOT/tauri.conf.json"' in launcher
assert 'dev --config "$TAURI_CONF"' in launcher
assert '"beforeDevCommand": "npm run dev"' in launcher
assert '"beforeBuildCommand": "npm run build"' in launcher
assert "npm --prefix frontend run dev" not in launcher
assert "npm --prefix frontend run build" not in launcher
assert 'cfg="${BACKUP_SYNC_CONFIG:-$(config_dir)/backup_sync/config.toml}"' in launcher
assert 'exec env BACKUP_SYNC_CONFIG="$cfg" BACKUP_SYNC_LOG="$log" "$DAEMON_BIN"' in launcher
assert 'start_daemon_in_background >/dev/null 2>&1 &' in launcher
assert 'DAEMON_PID="$!"' in launcher
PY

echo "launcher/config consistency checks passed"

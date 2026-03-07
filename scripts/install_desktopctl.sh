#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="$ROOT/tools/desktopctl/window_info.swift"
INSTALL_ROOT="${HOME}/.codex-desktop-bridge"
OUTPUT="${INSTALL_ROOT}/window-info"

error() {
  echo "Error: $1" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || error "Required command '$1' was not found on PATH."
}

if [ "$(uname -s)" != "Darwin" ]; then
  error "scripts/install_desktopctl.sh currently supports macOS only because window_info.swift depends on Quartz."
fi

[ -f "$SOURCE" ] || error "Missing Swift helper source at $SOURCE."
require_command swiftc

mkdir -p "$INSTALL_ROOT" || error "Failed to create install directory at $INSTALL_ROOT."

echo "Compiling desktop bridge helper from $SOURCE"
swiftc -O "$SOURCE" -framework CoreGraphics -framework Foundation -o "$OUTPUT" \
  || error "swiftc failed to build the window helper."

chmod 0755 "$OUTPUT" || error "Failed to mark $OUTPUT executable."

echo "Installed desktop bridge helper to $OUTPUT"

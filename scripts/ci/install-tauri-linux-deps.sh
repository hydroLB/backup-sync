#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "[tauri-deps] skipped: Linux packages are not required on this host"
  exit 0
fi

if ! command -v apt-get >/dev/null 2>&1; then
  echo "[tauri-deps] apt-get is required on Linux CI runners" >&2
  exit 1
fi

sudo apt-get update
sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential \
  curl \
  file \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  wget

echo "[tauri-deps] installed Tauri 2 Linux build prerequisites"

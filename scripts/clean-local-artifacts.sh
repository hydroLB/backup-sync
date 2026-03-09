#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

paths=(
  ".npm-cache"
  ".reports"
  "target"
)

for path in "${paths[@]}"; do
  if [[ -e "$path" ]]; then
    rm -rf "$path"
    echo "[clean-local] removed $path"
  fi
done

while IFS= read -r file; do
  rm -f "$file"
  echo "[clean-local] removed $file"
done < <(find . \
  -path './.git' -prune -o \
  -path './node_modules' -prune -o \
  -path './target' -prune -o \
  -name '.DS_Store' -type f -print)

#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
checker="${repo_root}/scripts/check-docs.py"
fixture_root="$(mktemp -d)"
trap 'rm -rf "${fixture_root}"' EXIT

mkdir -p "${fixture_root}/valid/docs"
cat >"${fixture_root}/valid/README.md" <<'EOF'
# Project

Read the [setup guide](docs/guide.md#setup). External [links](https://example.com) are ignored.
EOF
cat >"${fixture_root}/valid/docs/guide.md" <<'EOF'
# Guide

## Setup
EOF
python3 "${checker}" --root "${fixture_root}/valid" README.md >/dev/null

mkdir -p "${fixture_root}/missing-target"
cat >"${fixture_root}/missing-target/README.md" <<'EOF'
# Project

[Missing](docs/missing.md)
EOF
if python3 "${checker}" --root "${fixture_root}/missing-target" README.md >/dev/null 2>&1; then
  echo "[docs-smoke] expected a missing target to fail" >&2
  exit 1
fi

mkdir -p "${fixture_root}/missing-anchor/docs"
cat >"${fixture_root}/missing-anchor/README.md" <<'EOF'
# Project

[Missing anchor](docs/guide.md#not-there)
EOF
cat >"${fixture_root}/missing-anchor/docs/guide.md" <<'EOF'
# Guide

## Setup
EOF
if python3 "${checker}" --root "${fixture_root}/missing-anchor" README.md >/dev/null 2>&1; then
  echo "[docs-smoke] expected a missing anchor to fail" >&2
  exit 1
fi

echo "[docs-smoke] OK: valid links pass and broken targets/anchors fail"

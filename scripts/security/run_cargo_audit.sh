#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

# Vulnerabilities remain fatal. Informational advisories (for example,
# unmaintained GTK3 bindings pulled in only on Linux by Tauri) are surfaced in
# the report but do not make an otherwise patched cross-platform build fail.
args=(audit)
while IFS= read -r advisory; do
  args+=(--ignore "$advisory")
done < <(sed -n '/^\[advisories\]/,/^\[/p' deny.toml | rg -o 'RUSTSEC-[0-9]{4}-[0-9]{4}' | sort -u)

cargo "${args[@]}"

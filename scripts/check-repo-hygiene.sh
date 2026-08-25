#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[repo-hygiene] required command not found: $cmd"
    exit 1
  fi
}

require_cmd git
require_cmd rg
require_cmd tr
require_cmd wc

required_files=(
  "README.md"
  "SECURITY.md"
  ".gitignore"
  ".env.example"
  "Cargo.lock"
  "crates/gui/frontend/package-lock.json"
)

for file in "${required_files[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "[repo-hygiene] required restore-critical file missing: $file"
    exit 1
  fi
done

forbidden_tracked_paths_regex='(^|/)(\.DS_Store|Thumbs\.db|Desktop\.ini|\.env($|\.)|\.codex/|\.idea/|\.vscode/|projects/|\.reports/|\.npm-cache/|node_modules/|target/|coverage/|dist/|backup_sync/|crates/gui/gen/|.*\.(sqlite|sqlite3|db|pem|key|p12|pfx|crt|cer|der))($|/)'
forbidden_tracked_paths="$(git ls-files | rg "$forbidden_tracked_paths_regex" || true)"
if [[ -n "$forbidden_tracked_paths" ]]; then
  echo "[repo-hygiene] forbidden tracked local or secret-bearing paths detected:"
  echo "$forbidden_tracked_paths"
  exit 1
fi

machine_paths="$(
  while IFS= read -r path; do
    [[ -f "$path" ]] || continue
    [[ "$path" == "scripts/check-repo-hygiene.sh" ]] && continue
    [[ "$path" =~ \.(png|jpg|jpeg|gif|ico|icns|pdf|lock|sarif|json)$ ]] && continue
    rg -n --no-heading '/Users/|/home/|C:\\Users\\' "$path" || true
  done < <(git ls-files)
)"
filtered_machine_paths="$(printf '%s\n' "$machine_paths" | rg -v '/Users/me/|/home/user/|C:\\Users\\user|C:\\Users\\me' || true)"
if [[ -n "${filtered_machine_paths//$'\n'/}" ]]; then
  echo "[repo-hygiene] machine-specific absolute paths detected in tracked text files:"
  echo "$filtered_machine_paths"
  exit 1
fi

large_files="$(
  while IFS= read -r path; do
    [[ -f "$path" ]] || continue
    size="$(wc -c < "$path" | tr -d '[:space:]')"
    if (( size > 5242880 )); then
      printf '%s (%s bytes)\n' "$path" "$size"
    fi
  done < <(git ls-files)
)"
if [[ -n "$large_files" ]]; then
  echo "[repo-hygiene] tracked files larger than 5 MiB detected:"
  echo "$large_files"
  exit 1
fi

generated_comment_templates="$(
  rg -n \
    --glob '*.{rs,ts,tsx,js,jsx}' \
    '^\s*(\*|///) (Summary|Inputs|Outputs|Side effects|Error handling|Ties to other methods|Why this exists):' \
    crates || true
)"
if [[ -n "$generated_comment_templates" ]]; then
  echo "[repo-hygiene] generated comment templates detected; keep only non-obvious contracts or rationale:"
  echo "$generated_comment_templates"
  exit 1
fi

echo "[repo-hygiene] OK: tracked files are public-safe, restore-critical files exist, and repository bloat checks passed"

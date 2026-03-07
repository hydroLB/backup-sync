#!/usr/bin/env bash
set -euo pipefail

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[boundaries] required command not found: $cmd"
    exit 1
  fi
}

allowed_targets_for() {
  case "$1" in
    backup_core) echo "" ;;
    daemon) echo "backup_core" ;;
    cli) echo "backup_core" ;;
    gui) echo "backup_core tauri-plugin-single-instance" ;;
    *) return 1 ;;
  esac
}

is_policy_crate() {
  case "$1" in
    backup_core|daemon|cli|gui) return 0 ;;
    *) return 1 ;;
  esac
}

check_forbidden_imports() {
  local crate_name="$1"
  local crate_path="$2"
  local forbidden_pattern="$3"

  local targets=("$crate_path/src")
  if [[ -d "$crate_path/tests" ]]; then
    targets+=("$crate_path/tests")
  fi

  local output
  output="$(rg -n --glob '*.rs' "$forbidden_pattern" "${targets[@]}" || true)"
  if [[ -n "$output" ]]; then
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      echo "forbidden import in $crate_name: $line" >> "$VIOLATIONS_FILE"
    done <<< "$output"
  fi
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

require_cmd cargo
require_cmd jq
require_cmd rg
require_cmd tsort

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

METADATA_FILE="$TMP_DIR/metadata.json"
WORKSPACE_NAMES_FILE="$TMP_DIR/workspace_names.txt"
WORKSPACE_EDGES_FILE="$TMP_DIR/workspace_edges.txt"
POLICY_EDGES_FILE="$TMP_DIR/policy_edges.txt"
VIOLATIONS_FILE="$TMP_DIR/violations.txt"

cargo metadata --format-version 1 --no-deps > "$METADATA_FILE"

jq -r '
  . as $m
  | [ $m.workspace_members[] as $id | $m.packages[] | select(.id == $id) | .name ]
  | .[]
' "$METADATA_FILE" | sort -u > "$WORKSPACE_NAMES_FILE"

for crate in backup_core daemon cli gui; do
  if ! rg -qx "$crate" "$WORKSPACE_NAMES_FILE" >/dev/null; then
    echo "[boundaries] expected workspace crate '$crate' was not found"
    exit 1
  fi
done

jq -r '
  . as $m
  | [ $m.workspace_members[] as $id | $m.packages[] | select(.id == $id) ] as $workspace_packages
  | [ $workspace_packages[].name ] as $workspace_names
  | $workspace_packages[]
  | .name as $from
  | .dependencies[]
  | .name as $to
  | select((.path // "") != "")
  | select($workspace_names | index($to))
  | "\($from) \($to)"
' "$METADATA_FILE" | sort -u > "$WORKSPACE_EDGES_FILE"

: > "$VIOLATIONS_FILE"
: > "$POLICY_EDGES_FILE"

while IFS=' ' read -r from to; do
  [[ -z "${from:-}" ]] && continue

  if is_policy_crate "$from"; then
    allowed_targets="$(allowed_targets_for "$from")"
    if [[ -z "$allowed_targets" ]]; then
      echo "forbidden workspace dependency: $from -> $to (allowed: none)" >> "$VIOLATIONS_FILE"
    else
      is_allowed=0
      for allowed_target in $allowed_targets; do
        if [[ "$to" == "$allowed_target" ]]; then
          is_allowed=1
          break
        fi
      done
      if [[ "$is_allowed" -eq 0 ]]; then
        echo "forbidden workspace dependency: $from -> $to (allowed: $allowed_targets)" >> "$VIOLATIONS_FILE"
      fi
    fi
  fi

  if is_policy_crate "$from" && is_policy_crate "$to"; then
    echo "$from $to" >> "$POLICY_EDGES_FILE"
  fi
done < "$WORKSPACE_EDGES_FILE"

if [[ -s "$POLICY_EDGES_FILE" ]]; then
  if ! tsort "$POLICY_EDGES_FILE" >/dev/null 2> "$TMP_DIR/tsort.err"; then
    cycle_err="$(tr '\n' ' ' < "$TMP_DIR/tsort.err" | sed 's/[[:space:]]\+/ /g')"
    echo "cycle detected in enforced crates: ${cycle_err}" >> "$VIOLATIONS_FILE"
  fi
fi

check_forbidden_imports "backup_core" "crates/core" "\\b(daemon|cli|gui_app)::"
check_forbidden_imports "daemon" "crates/daemon" "\\b(cli|gui|gui_app)::"
check_forbidden_imports "cli" "crates/cli" "\\b(daemon|gui|gui_app)::"
check_forbidden_imports "gui" "crates/gui" "\\b(daemon|cli)::"

if [[ -s "$VIOLATIONS_FILE" ]]; then
  echo "[boundaries] dependency policy violations detected:"
  while IFS= read -r violation; do
    echo "  - $violation"
  done < "$VIOLATIONS_FILE"
  exit 1
fi

echo "[boundaries] OK: layer dependencies and import boundaries satisfy policy"

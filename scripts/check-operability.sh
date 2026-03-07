#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[operability] required command not found: $cmd"
    exit 1
  fi
}

require_file() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "[operability] required file missing: $file"
    exit 1
  fi
}

require_pattern() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if ! rg -q "$pattern" "$file"; then
    echo "[operability] missing enforcement point: $label ($file :: $pattern)"
    exit 1
  fi
}

require_cmd rg

require_file "docs/operability.md"
require_file "docs/performance-tuning.md"
require_file "crates/core/src/config/validate/sections/tuning.rs"
require_file "crates/daemon/src/runtime/ipc.rs"
require_file "crates/daemon/src/runtime/loop.rs"
require_file "crates/daemon/tests/ipc_load.rs"
require_file "crates/core/src/backup/versioned/store.rs"
require_file "crates/gui/src/commands/service/common.rs"

require_pattern "crates/core/src/config/validate/sections/tuning.rs" "execution.retry_delays_ms must be non-decreasing" "retry backoff monotonic validation"
require_pattern "crates/core/src/config/validate/sections/tuning.rs" "runtime.ipc_timeout_seconds" "runtime IPC timeout validation"
require_pattern "crates/core/src/config/validate/sections/tuning.rs" "runtime.daemon_shutdown_join_timeout_seconds" "shutdown join timeout validation"

require_pattern "crates/daemon/src/runtime/ipc.rs" "HealthWithContext" "IPC health request contract"
require_pattern "crates/daemon/src/runtime/ipc.rs" "ReadinessWithContext" "IPC readiness request contract"
require_pattern "crates/daemon/src/runtime/ipc.rs" "write_health_reply" "health reply writer"
require_pattern "crates/daemon/src/runtime/ipc.rs" "write_readiness_reply" "readiness reply writer"
require_pattern "crates/daemon/src/runtime/ipc.rs" "timeout\(ipc_timeout" "bounded IPC read/write timeout usage"
require_pattern "crates/daemon/tests/ipc_load.rs" "unix_ipc_health_load_contract" "IPC load test contract"
require_pattern "crates/daemon/tests/ipc_load.rs" "BACKUP_SYNC_IPC_LOAD_MAX_P95_MS" "IPC load test p95 threshold override"
require_pattern "docs/performance-tuning.md" "Resource knobs and guardrails" "resource tuning documentation"

require_pattern "crates/daemon/src/runtime/loop.rs" "join_task_with_timeout" "graceful shutdown join coordinator"
require_pattern "crates/daemon/src/runtime/loop.rs" "handle.abort\(\)" "forced shutdown fallback"

require_pattern "crates/core/src/backup/versioned/store.rs" "retry_with_backoff" "core retry/backoff helper"
require_pattern "crates/gui/src/commands/service/common.rs" "run_command_with_retry" "service retry enforcement helper"

echo "[operability] OK: health/readiness, shutdown, and timeout/retry/backoff enforcement points present"

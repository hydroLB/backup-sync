#!/bin/sh
set -e

attempts="${BACKUP_SYNC_PERF_ATTEMPTS:-5}"

run_with_retries() {
  label="$1"
  shift
  attempt=1
  while [ "$attempt" -le "$attempts" ]; do
    if "$@"; then
      return 0
    fi
    if [ "$attempt" -lt "$attempts" ]; then
      echo "perf-check ${label} attempt ${attempt}/${attempts} failed; retrying..."
      sleep 1
    fi
    attempt=$((attempt + 1))
  done
  echo "perf-check ${label} failed after ${attempts} attempts"
  return 1
}

run_with_retries "core-hot-paths" cargo run -p backup_core --bin perf_guard -- --baseline docs/perf-baseline.json --max-ratio 1.5
run_with_retries "ipc-load" make perf-ipc-load

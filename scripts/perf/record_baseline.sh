#!/bin/sh
set -e

cargo run -p backup_core --bin perf_guard -- --record --baseline docs/perf-baseline.json

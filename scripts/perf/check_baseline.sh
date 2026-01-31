#!/bin/sh
set -e

cargo run -p backup_core --bin perf_guard -- --baseline docs/perf-baseline.json --max-ratio 1.5

#!/usr/bin/env bash
# Runs the Flux render-perf harness (PRD-J) as a CI step.
#
# Builds flux-perf-harness and executes the demonstration runner, which builds a
# fixed warm sample record and runs the §3.10 budget gate end to end, emitting a
# stable JSON MetricRecord. The hard pass/fail budget gate is enforced by the
# crate's own unit tests (gate::tests) and by host-adapter CI once the
# runtimes/ measurement wires in.
#
# Usage: scripts/run-perf-harness.sh [--json OUT_FILE]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

JSON_OUT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --json) JSON_OUT="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

echo "== building flux-perf-harness =="
cargo build -p flux-perf-harness --example ci_run

echo "== running perf-harness demonstration =="
if [[ -n "$JSON_OUT" ]]; then
  cargo run -p flux-perf-harness --example ci_run | tee >(grep '^metric_record_json:' | sed 's/^metric_record_json://' > "$JSON_OUT")
else
  cargo run -p flux-perf-harness --example ci_run
fi

echo "== harness unit tests (hard budget gate) =="
cargo test -p flux-perf-harness --lib

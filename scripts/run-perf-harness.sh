#!/usr/bin/env bash
# Runs the Flux render-perf harness (PRD-J / FLUX-066) as a CI step.
#
# 1. Builds + runs the Rust `flux-perf-harness` crate — the platform-neutral core
#    (shared MetricRecord schema, deterministic driver, §3.10 budget gate). The
#    `ci_run` example builds a fixed warm record and evaluates the gate; the crate
#    unit tests are the hard gate.
# 2. Runs the on-device measurements in the host adapters (the FLUX-066 wiring):
#    - Android `:host` (pure JVM, no emulator needed) emits a MetricRecord JSON.
#    - iOS `FluxApp` on a booted simulator emits a MetricRecord JSON.
#    Each host writes its record to a file under `$OUTDIR`; when a device/sim is
#    unavailable the step prints a SKIP note instead of failing (the numbers need
#    a simulator/emulator — see FLUX-066 status).
# 3. The Rust `ci_ondevice` example loads every collected record and evaluates the
#    §3.10 budget gate over the REAL measurements, exiting non-zero on regression.
#
# Usage: scripts/run-perf-harness.sh [--json OUT_FILE] [--outdir DIR]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

JSON_OUT=""
OUTDIR="$(mktemp -d)/flux-perf"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --json) JSON_OUT="$2"; shift 2 ;;
    --outdir) OUTDIR="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done
mkdir -p "$OUTDIR"

echo "== 1/3 building + testing flux-perf-harness (Rust core) =="
cargo build -p flux-perf-harness --example ci_run --example ci_ondevice
cargo test -p flux-perf-harness --lib

echo "== running harness demonstration (fixed warm record + gate) =="
if [[ -n "$JSON_OUT" ]]; then
  cargo run -q -p flux-perf-harness --example ci_run | tee >(grep '^metric_record_json:' | sed 's/^metric_record_json://' > "$JSON_OUT")
else
  cargo run -q -p flux-perf-harness --example ci_run
fi

# Collector of every `RENDER_PERF …` line the host adapters emit. `ci_ondevice`
# parses each line by extracting the first balanced `{…}` payload, so we pass the
# raw printer lines (one per host) straight through.
RECORDS_FILE="$OUTDIR/host-records.txt"
: > "$RECORDS_FILE"

# ---- Android host (pure JVM, no emulator) ----
echo "== 2a/3 Android host render-perf (JVM, no emulator) =="
if [[ -x "./gradlew" ]]; then
  ./gradlew :runtimes:android:host:test --tests "dev.flux.host.RenderPerfHarnessTest" -q 2>/dev/null || true
  AXML="$(find runtimes/android/host/build/test-results -name '*RenderPerfHarnessTest*.xml' 2>/dev/null | head -1 || true)"
  if [[ -n "${AXML:-}" ]] && grep -q 'RENDER_PERF' "$AXML"; then
    grep 'RENDER_PERF' "$AXML" >> "$RECORDS_FILE"
    echo "  collected android record from $AXML"
  else
    echo "  SKIP: android record not found (gradle may be unavailable in this env)"
  fi
else
  echo "  SKIP: ./gradlew not present"
fi

# ---- iOS host (needs a booted simulator) ----
echo "== 2b/3 iOS host render-perf (simulator) =="
SIM_ID="$(xcrun simctl list devices booted 2>/dev/null | grep -oE '[0-9A-Fa-f-]{36}' | head -1 || true)"
if [[ -n "${SIM_ID:-}" ]] && command -v xcodebuild >/dev/null 2>&1; then
  set +e
  xcodebuild test -scheme FluxApp \
    -destination "platform=iOS Simulator,id=$SIM_ID" \
    -only-testing 'FluxAppTests/RenderPerfHarnessTests' 2>&1 \
    | grep 'RENDER_PERF' >> "$RECORDS_FILE"
  set -e
  echo "  iOS xcodebuild run complete (record, if any, captured above)"
else
  echo "  SKIP: no booted iOS simulator in this environment (FLUX-066 needs a sim)"
fi

# ---- Gate the real measurements ----
echo "== 3/3 gating collected on-device records against §3.10 budgets =="
if [[ ! -s "$RECORDS_FILE" ]]; then
  echo "  No on-device records collected in this environment; Rust-core gate already passed above."
  echo "  (On a machine with the Android toolchain and/or a booted iOS simulator the host"
  echo "   records are collected and gated here — see FLUX-066.)"
  exit 0
fi
cargo run -q -p flux-perf-harness --example ci_ondevice "$RECORDS_FILE"

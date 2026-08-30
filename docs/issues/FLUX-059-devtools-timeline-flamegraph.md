---
id: FLUX-059
status: implemented
lane: LANE-P
phase: "Phase 4"
blocked_by: []
labels:
  - devtools
  - perf
source: CHANGELOG.md §PRD-P (deferred: "timeline/flamegraph ingesting PRD-J's MetricRecord (user stories 3 & 8)")
related_adrs:
  - ADR-0040
---

# FLUX-059: DevTools timeline / flamegraph from PRD-J MetricRecord

- **Lane:** LANE-P (Phase 4)
- **Depends on:** PRD-J (`MetricRecord` schema) — DevTools and CI share one source
  of truth (landed as DONE: `crates/flux-perf-harness`, `MetricRecord`)
- **Source:** `CHANGELOG.md` §PRD-P deferred (user stories 3 & 8)
- **Related ADRs:** ADR-0040 (host instrumentation)

## Status (2026-08-30) — IMPLEMENTED

The blocker named in the original issue (`blocked_by: PRD-J`, "no `MetricRecord`-
emitting telemetry event variant lands") is resolved. FLUX-059 now:

- Consumes `flux-perf-harness`'s `MetricRecord` **verbatim** — no new wire field.
  The record travels as its stable JSON (`MetricRecord::to_json`) inside a new
  `PerfRecord` variant on the existing `0x10` telemetry frame
  (`flux-ir-serde::TelemetryEvent::PerfRecord` / `EnrichedTelemetryEvent::PerfRecord`,
  tag `0x07`). A `TelemetryEvent::perf_record(json)` constructor builds it.
- `DevToolsState::ingest_perf_record` parses each `PerfRecord` JSON into a
  `MetricRecord` and stores it (bounded ring buffer, malformed JSON dropped with a
  warning, never panics). `handle_telemetry` routes `PerfRecord` events straight
  into it, so the live wire path needs no extra glue.
- The `timeline` pane (`views/timeline.rs`) now renders a **budget-aware
  flamegraph** (`perf_record.rs`): one lane per `(Scenario, MetricKind)`, each
  bar's width = p95 ÷ §3.10 ceiling, green when within budget / red when over
  (using the same `Budgets::v1()` CI gates PRD-J enforces). Header shows record
  count / lane count / over-budget count. Empty until the first `PerfRecord`
  arrives (honest empty state, no fabricated bars).

### Broadcast follow-up (DONE)

- **`flux-devserver` now broadcasts `PerfRecord` frames.** `Pipeline::perf_records()`
  builds the server-side `Save → pixels` breakdown from the real per-compile
  `PhaseTimings` (parse+type_check+lower+diff+serialize → `MetricKind::SaveToPhoton`;
  serialize alone → `MetricKind::PatchRoundTrip`) with the lowered node count as
  `tree_size`, `Scenario::LoopbackE2e`. `compile_and_broadcast` (watcher hot path) and
  `initial_compile` both broadcast each record as `TelemetryEvent::perf_record(json)`
  via `DevToolsRouter::route_telemetry` on `:7333`, so the flamegraph fills from the
  first compile. `flux-perf-harness` promoted from dev-dep to a regular dep of
  `flux-devserver`. Tests: `pipeline::tests::perf_records_*` +
  `debug_bridge::tests::router_broadcasts_perf_record_to_subscribers`.

### Verification note
  scope) which broke `flux-ir`, a transitive dep of `flux-devtools-ui`. Independent
  layers also green: `flux-perf-harness` tests green; `flux-ir-serde` (wire variant +
  round-trip tests) clippy-clean + all suites green. No fabricated green.

## Problem Statement

PRD-P deferred "timeline/flamegraph ingesting PRD-J's `MetricRecord`": patch
dispatch latency, VM instruction timing, dirty-reconciliation size per frame. The
harness emits the records; DevTools doesn't render them.

## Solution

Render a timeline/flamegraph in the `timeline` view from the `MetricRecord` stream
the dev server already emits (PRD-J). DevTools and CI perf gates read the same
schema.

## Implementation Decisions

- Consumes `flux-perf-harness`'s `MetricRecord` verbatim — no new wire field.
- The `time_travel` ring buffer (shipped) provides the scrubber backing store.

## Testing Decisions

- A fixture `MetricRecord` stream renders the expected timeline bars; reuse the
  devtools integration test path.

## Out of Scope

- The signal-graph view (FLUX-058), network inspector (FLUX-060).

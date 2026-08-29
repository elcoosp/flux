---
id: FLUX-059
status: todo
lane: LANE-P
phase: "Phase 4"
blocked_by:
  - PRD-J
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
  of truth
- **Source:** `CHANGELOG.md` §PRD-P deferred (user stories 3 & 8)
- **Related ADRs:** ADR-0040

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

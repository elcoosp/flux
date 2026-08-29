---
id: FLUX-058
status: todo
lane: LANE-P
phase: "Phase 4"
blocked_by: []
labels:
  - devtools
  - signal-graph
source: CHANGELOG.md §PRD-P (deferred: "signal-graph dependency-edge rendering (user story 2)")
related_adrs:
  - ADR-0040
---

# FLUX-058: DevTools signal-graph dependency-edge rendering

- **Lane:** LANE-P (Phase 4)
- **Depends on:** none (telemetry `SignalGraph.write` already instruments)
- **Source:** `CHANGELOG.md` §PRD-P deferred (user story 2)
- **Related ADRs:** ADR-0040 (host instrumentation)

## Problem Statement

PRD-P deferred "signal-graph dependency-edge rendering (user story 2)": the live
visualization of the SolidJS-style dependency graph with "what wrote this signal"
and "what reads it." This is a genuine differentiator (neither RN nor Flutter has
it natively) and is currently scaffolded, not shipped.

## Solution

Render the dependency edges in `flux-devtools-ui`'s `signal_graph` view from the
telemetry `SignalGraph.write` events: a node per signal, an edge per
read/write dependency, with click-to-inspect (writer + readers). The core event
stream already flows to DevTools (ADR-0039/0040); this is the view.

## Implementation Decisions

- Pure function over the telemetry events; unit-testable without a socket.
- Reuses the PRD-P UI-free `SourceMap` span plumbing for click-to-source.

## Testing Decisions

- A fixture signal graph (known writes/reads) renders the expected edge set; the
  existing 19 devtools tests cover the data path.

## Out of Scope

- Timeline/flamegraph (FLUX-059), network inspector (FLUX-060), multi-device
  (FLUX-061), on-device demo evidence (FLUX-062).

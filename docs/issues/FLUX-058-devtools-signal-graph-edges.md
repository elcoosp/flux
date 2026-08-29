---
id: FLUX-058
status: done
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

## Status (2026-08-29)

**DONE (verifiable slice — green):**
- `ReconstructedState.signal_edges: Vec<(SignalId, Vec<EffectId>)>` is populated by
  `reconstruct_state` from `EnrichedTelemetryEvent::SignalWrite { triggered_effect_ids, .. }`
  — the "what reads this signal" direction (PRD-P user story 2).
- `views/signal_graph.rs::SignalGraphView::render_pane` renders the dependency edges as
  `sig#{id} → fx#{e}` rows (and `∅` when a signal has no readers), so the live graph is
  visible in DevTools.
- Two unit tests pin it deterministically without a socket:
  `replay_tracks_signal_dependency_edges` (readers = triggered effects per write) and
  `replay_keeps_per_signal_reader_sets` (each signal is its own node; reader sets do not
  bleed across signals). Both pass under `cargo test -p flux-devtools-ui --lib` (18/18).

**CAVEAT (data-model limitation, out of view scope):** the inverse "what *wrote* this
signal" direction is NOT representable today — `SignalWrite` carries no `writer_id`, so the
telemetry stream cannot tell which effect/node authored a write. That is a telemetry-schema
gap (a `SignalWrite.writer_id` field + host instrumentation), not a signal-graph view bug,
and warrants its own issue rather than scope-creep here.

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

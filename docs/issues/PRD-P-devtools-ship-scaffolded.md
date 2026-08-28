---
id: PRD-P
status: open
lane: LANE-P
phase: "Phase 4"
blocked_by:
  - PRD-K
  - PRD-J
labels:
  - epic
  - prd
  - devtools
  - gpui
  - observability
  - ios
  - android
source: docs/roadmaps/flux-roadmap-to-1.0.md §4,§12,§13
related_adrs:
  - ADR-0041
  - ADR-0042
---

# PRD-P: DevTools — Ship What Is Scaffolded

- **Lane:** LANE-P (Phase 4)
- **Depends on:** PRD-K (spans), PRD-J (perf instrumentation)
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §4, §12, §13
- **Related ADRs:** ADR-0041 (gpui DevTools UI), ADR-0042 (time-travel store), PRD-J (perf harness),
  PRD-K (span-threaded errors)

## Problem Statement

`flux-devtools-ui` (gpui) has the right module skeleton — `time_travel`, `views/{component_tree,
signal_graph,timeline,vm_inspector}`, `wire_client` — but it is a skeleton: no error overlay hookup, no
network inspector, no perf flamegraphs, no on-device connection validated against a real app. The
roadmap is explicit: ship it, not scaffold it. Flux's signal-graph-native architecture is a genuine
differentiator (neither RN nor Flutter has native signal-graph visualization), so a shipped
component/signal/timeline/time-travel DevTools is high-leverage.

## Solution

Take each scaffolded DevTools module from scaffold to shipped: component tree (click a node → jump to
its `.flux` source line, needs PRD-K span-threading), signal graph (live SolidJS-style dependency
visualization with "what wrote / what reads"), timeline/flamegraph (patch dispatch latency, VM
instruction timing, dirty-reconcile size per frame — fed from PRD-J's harness so DevTools and CI share
one source of truth), time-travel (scrub signal-graph history + replay, demoed against a real app). Add
a network inspector (once PRD-Q's HTTP capability exists) and a structured log viewer (ties into
`tracing`). Add multi-device: connect DevTools to more than one running host at once.

## User Stories

1. As a Fluff app developer, I want to click a component-tree node and jump to its `.flux` source line,
   so that I can navigate from what I see to where it is defined.
2. As a Fluff app developer, I want a live signal graph showing what wrote a signal and what reads it,
   so that I can reason about reactivity the way the VM actually works.
3. As a Fluff app developer, I want a timeline/flamegraph of patch dispatch latency, VM instruction
   timing, and dirty-reconcile size per frame, so that I can see perf regressions as I code.
4. As a Fluff app developer, I want time-travel: scrub signal-graph history and replay, so that I can
   reproduce a state without re-clicking my app.
5. As a Fluff app developer, I want a network inspector once HTTP exists, so that capability calls are
   visible like any other network client.
6. As a Fluff app developer, I want a structured log viewer tied to `tracing`, so that host logs are
   queryable, not a scrollback.
7. As a Fluff app developer, I want DevTools connected to iOS + Android simultaneously, so that I can
   compare both platforms side by side.
8. As a Flux core engineer, I want DevTools to consume PRD-J's perf metrics and PRD-K's spans, so that
   DevTools and CI share one instrumentation source of truth.

## Implementation Decisions

- **One instrumentation source of truth:** the timeline/flamegraph reads the same metric record PRD-J's
  harness emits, so a regression shown in DevTools is the same number that fails CI. Do not build a
  second profiler.
- **Spans drive navigation:** component-tree → source jump and the (future) error overlay both rely on
  PRD-K's span-threaded `FluxError`/`Span`; DevTools does not re-derive source locations.
- **gpui shell stays:** `flux-devtools-ui` is a gpui app (needs the nightly toolchain per AGENTS.md §0.3);
  this PRD fills the existing module skeletons, it does not change the framework choice.
- **Time-travel needs a real demo:** ADR-0042's `time_travel/{buffer,reconstruct}` is designed but must
  be demoed end-to-end against a running app before this item is "done" — a unit test is not sufficient
  evidence per the roadmap's "ship it, not scaffold it" bar.
- **Network inspector gated on PRD-Q:** it is listed as a *new* module here but only becomes meaningful
  once the HTTP capability lands; track the inspector wiring here, the capability in PRD-Q.

## Testing Decisions

- **Good test:** an integration test driving DevTools against a headless dev VM session and asserting the
  component tree, signal graph, and timeline reflect a known fixture; a time-travel test asserting
  replay reconstructs a known prior state. Not tests of gpui widget internals.
- **Modules to test:** `views/component_tree` (span → source resolve), `views/signal_graph` (dependency
  edges), `views/timeline` (metric ingestion from PRD-J), `time_travel/{buffer,reconstruct}`, and
  `wire_client` (multi-device connect).
- **Prior art:** ADR-0042's time-travel store design and the `flux-parity` trace format are the seed;
  reuse the trace schema for signal-graph edges.

## Out of Scope

- Building the perf harness (PRD-J) — DevTools consumes it.
- The error taxonomy / span-threading (PRD-K) — DevTools consumes it.
- The HTTP capability itself (PRD-Q).
- The on-device error overlay (PRD-O) — separate surface, shares spans.

## Further Notes

PRD-P is the concrete deliverable behind roadmap §1.3 "DevTools parity+." It is explicitly sequenced
*after* PRD-K and PRD-J because both feed it; starting it earlier would mean building against spans and
metrics that do not yet exist.

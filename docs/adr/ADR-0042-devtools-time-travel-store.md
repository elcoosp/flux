# ADR-0042 — DevTools time-travel data store

- Status: Accepted
- Date: 2026-08-26
- Scope: `flux-devtools-ui` (time-travel core), `flux-devserver` (snapshot source)
- Supersedes: none
- Superseded by: none
- Related: ADR-0039, DevTools spec §6 (ADR-0033 in the spec is renumbered here
  to 0042 to avoid colliding with the already-accepted ADR-0033
  `flux018-string-table-gap`)

## Context

Time-travel debugging requires storing a history of state changes so the user
can scrub backward and forward through VM execution and signal writes. A naive
"store every full state" approach blows memory for a long session.

## Decision

Implement a fixed-capacity **ring buffer** (`TimelineBuffer`) of
`EnrichedTelemetryEvent`s in the DevTools app. When the buffer is full the
oldest event is dropped (bounded memory). The host periodically emits a
`RequestSnapshot`/`base-state` snapshot (a compact `EnrichedTelemetryEvent`
carrying the full current `VmState` + `SignalGraphSnapshot` + `ViewTreeSnapshot`)
so the app can reconstruct any point in time by:

1. Finding the nearest base-state snapshot at or before the target index.
2. Replaying every `TelemetryEvent` from that snapshot to the target index
   through a local, allocation-light state simulator (`reconstruct_state`).

State reconstruction is pure (no I/O) and tested independently of gpui so the
algorithm is verifiable in CI. The ring buffer capacity defaults to 10_000
events (configurable).

## Consequences

- Bounded memory regardless of session length.
- Deterministic scrubbing: replay is a pure fold over the event slice.
- The dev server must forward/emit base snapshots; Phase 3 wires this through
  the existing telemetry path.

## Alternatives considered

- **Unbounded `Vec`**: rejected — memory growth unbounded over a debugging
  session.
- **Full-state-per-event**: rejected — O(n·state) memory; the ring buffer +
  periodic base snapshot gives O(capacity·state) with identical scrub fidelity
  for the retained window.

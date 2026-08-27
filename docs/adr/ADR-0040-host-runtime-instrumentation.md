# ADR-0040 — Host runtime telemetry instrumentation

- Status: Accepted
- Date: 2026-08-26
- Scope: `runtimes/ios` (`FluxBytecodeVM`, `SignalGraph`), `runtimes/android`
  (`FluxBytecodeVM`), wire bridge
- Supersedes: none
- Superseded by: none
- Related: ADR-0039, DevTools spec §3

## Context

The host VM and signal graph have no hooks exposing internal state. To stream
telemetry (ADR-0039) the host must observe VM instructions, signal writes,
view mutations and handler invocations without blocking the VM/UI thread.

## Decision

Add a `TelemetrySink` protocol (Swift `VMTelemetrySink` / Kotlin
`VMTelemetrySink`) that the VM and signal graph emit into via a weak/optional
reference. All emission sites are guarded:

- Swift: `#if DEBUG` around the protocol, the VM `telemetrySink` property, and
  every `emit` call site.
- Kotlin: the sink is an optional `var telemetrySink: VMTelemetrySink?` read on
  the hot path; in release builds it is `null` and the `?.` short-circuits to
  zero cost.

A thread-safe queue (`TelemetryBridge` on iOS, `TelemetryBridge` on Android)
collects events off the VM/UI thread and flushes them in batches over the
existing WebSocket connection, reusing the `Telemetry` frame encoder from
`flux-ir-serde` (the host apps depend on the shared wire layout; they encode
the same bytes the server decodes). Batching: flush when the queue reaches a
threshold (10 events) or a 16 ms window elapses.

## Consequences

- Zero release impact: the Swift guards compile the sink out entirely; the
  Kotlin sink is a nullable field that is never assigned in release.
- Asynchronous telemetry: the VM never blocks on a slow WebSocket.
- The dev server (`flux-devserver` `debug_bridge`, ADR-0042) becomes the single consumer and enricher.

## Alternatives considered

- **Emit synchronously from the VM**: rejected — would stall the VM on network
  I/O and violate the §3.3 "asynchronous telemetry" principle.
- **Per-event WebSocket send**: rejected — frame header overhead dominates for
  high-frequency `VmStep` events; batching is required by §3.3.

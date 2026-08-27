# ADR-0039 — DevTools bidirectional wire protocol extension

- Status: Accepted
- Date: 2026-08-26
- Scope: `flux-ir-serde` (Rust wire codec), `flux-devserver`, iOS/Android host apps
- Supersedes: none
- Superseded by: none
- Related: Appendix D §D.12, DevTools specification §2

## Context

The existing wire protocol (Appendix D) is unidirectional: the host app
connects to the dev server, receives `Init`/`Delta` frames, and ships `Hello`.
There is no path for the host to stream internal runtime state (VM steps,
signal writes, view mutations, handler invocations) to a desktop debugger, nor
for the debugger to control VM execution (pause/resume/step/breakpoint). The
DevTools suite (gpui desktop app) requires both directions.

The existing frame-type registry stops at `0x05` (`Heartbeat`). The spec
reserves bytes `0x10`/`0x11` for `Telemetry` and `DebugCommand` respectively.

## Decision

Extend Appendix D §D.12 with two new frame kinds:

1. **`Telemetry` (`0x10`, Host → Server)** — a batch of length-prefixed
   `TelemetryEvent`s (`VmStep`, `SignalWrite`, `ViewMutation`,
   `HandlerInvocation`). Layout per the DevTools spec §2.2:
   `MAGIC(4) version(1) kind(0x10) event_count(2) [events...]`.
2. **`DebugCommand` (`0x11`, Server → Host)** — a single control command
   (`Pause`, `Resume`, `Step`, `SetBreakpoint`, `ClearBreakpoint`,
   `RequestSnapshot`). Layout per the spec §2.3:
   `MAGIC(4) version(1) kind(0x11) command_id(4) payload_len(2) payload`.

Both frames reuse the existing `MAGIC`/`PROTOCOL_VERSION` header and the
`encode_value`/`decode_value` primitives already in `wire.rs`, so they stay
byte-compatible with the Swift/Kotlin production decoders' conventions.

The host emits raw IDs (`bytecode_offset`, `NodeId`). Source-span enrichment
happens **server-side** in the dev-server `debug_bridge` (ADR-0042), so the wire payload stays
tiny and the host stays release-clean (all instrumentation is `#if DEBUG` /
`BuildConfig.DEBUG` guarded).

## Consequences

- A desktop debugger can attach over a second WebSocket port (`:7333`) and
  receive a live telemetry stream enriched with `.flux` source spans.
- `flux-ir-serde` gains a `Telemetry`/`DebugCommand` codec living in a new
  `telemetry.rs` module, isolated from the in-progress `frame.rs` work so it
  does not collide with the concurrent FLUX-013/FA-IRWIRE frame edits.
- The Swift/Kotlin host `FrameDeserializer`s gain parallel decode arms for
  `0x10`/`0x11`, matching the same byte layout (the Swift/Kotlin `FrameDeserializer`s are
  implemented in `runtimes/ios` / `runtimes/android`).

## Alternatives considered

- **Repurpose `Heartbeat`'s reserved spare bytes**: rejected — telemetry needs
  a distinct, densely-packed event stream; co-opting an unrelated frame would
  break the heartbeat contract.
- **Tunnel telemetry over the existing `:7331` channel as `Delta` sub-payload**:
  rejected — mixes debug traffic with production patches; the production host
  decoder must never see debug-only bytes.

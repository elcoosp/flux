---
id: FLUX-060
status: done
lane: LANE-P
phase: "Phase 4"
blocked_by: []
labels:
  - devtools
  - network
source: CHANGELOG.md §PRD-P (deferred: "the network inspector + log viewer")
related_adrs:
  - ADR-0039
  - ADR-0044
---

# FLUX-060: DevTools network inspector + structured log viewer

- **Lane:** LANE-P (Phase 4)
- **Depends on:** FLUX-047 (HTTP capability exists to inspect) — or works on the
  wire frames in the meantime
- **Source:** `CHANGELOG.md` §PRD-P deferred
- **Related ADRs:** ADR-0039 (wire extension)

## Problem Statement

PRD-P deferred "the network inspector + log viewer." Once an HTTP capability exists
(FLUX-047) there is real network traffic to inspect; the `tracing` log stream
(already a workspace dep) is the log source.

## Solution

A network inspector view (request/response over the HTTP capability or the dev
server frames) + a structured log viewer bound to `tracing` output in DevTools.

## Implementation Decisions

- Network inspector reads the capability telemetry frames (no new wire field).
- Log viewer consumes the existing `tracing` stream the dev server already emits.

## Testing Decisions

- A fixture HTTP request renders in the inspector; a `tracing` event renders in the
  log viewer.

## Out of Scope

- Crash reporting (FLUX-035) — release concern.

## Status (2026-08-30)

**Log viewer — DONE (verified).** Pure, gpui-decoupled `LogBuffer` / `LogEntry` /
`LogLevel` model (`time_travel/log_buffer.rs`, 4 unit tests), wired into
`DevToolsState` (`logs: RwLock<LogBuffer>` + `ingest_log` / `log_snapshot`, 1 test).
The gpui `LogViewerView` (`views/log_viewer.rs`, 1 test) is re-exported from
`views/mod.rs` and mounted as the `Logs` pane in the `DevToolsRoot` 3×2 grid.
(The view had been dropped in `2647f09` and was not actually mounted; the earlier
"DONE" note was stale — the model existed but nothing rendered it.)

**Network inspector — DONE (unblocked by FLUX-047).** FLUX-047 (HTTP capability) is
now `done`, so real HTTP traffic exists to inspect. Added `NetworkRequest` /
`NetworkResponse` telemetry variants (`flux-ir-serde`, tags `0x05` / `0x06`,
length-prefixed, no new wire field) + a pure `NetworkLog` / `NetworkRecord` /
`NetworkPhase` model (`time_travel/network_log.rs`, 6 tests) that pairs a response
with its request by `request_id`. Wired into `DevToolsState` (`net` +
`ingest_network_request` / `ingest_network_response` / `network_snapshot`) through
the single `handle_telemetry` path, and rendered by `NetworkInspectorView`
(`views/network_inspector.rs`, 1 test) as the `Network` pane.

**Verification:** `cargo nextest run -p flux-devtools-ui --lib` → 45/45 green
(incl. all FLUX-060 tests); `cargo nextest run -p flux-ir-serde` → 68/68 green (incl.
the new network wire tests). `cargo clippy` is clean on all FLUX-060-owned files.

**Live broadcast — DONE (2026-08-30 follow-up).** Both hosts now emit
`NetworkRequest` / `NetworkResponse` around the `Http` capability (FLUX-047), so the
inspector shows real outbound traffic end-to-end, not just fixtures:
- Android `HttpCapabilities.makeHttpResolver` emits `TelemetryEvent.NetworkRequest`
  (capability id `14`) before the call and `TelemetryEvent.NetworkResponse`
  (`status_code`/`latency_ms`/`result_kind` = `1` Ready or `2` Error) after it.
- iOS `HttpAsyncResolver.resolve` does the same via `fluxDevtoolsEmit`.
- Wire layout is bit-identical across Kotlin/Swift/Rust (tag `0x05`/`0x06`,
  length-prefixed, `u16` string length prefix). Pinned by
  `telemetry_round_trip::host_network_telemetry_decodes_to_network_events` (Rust)
  and `TelemetryNetworkTest` (Android) against the same canonical byte array.
- Also corrected a version divergence: the Android host telemetry frame defaulted
  to `PROTOCOL_VERSION = 1` while `flux-ir-serde` requires `2`, so the dev server
  silently dropped host frames. Bumped Android `toFrameBytes` default to `0x02` to
  match iOS (`0x02`) and Rust.

Host Kotlin/Swift are not compiled in this environment (no Android toolchain; Swift
verified via `swiftc` for the emit-byte generator), but the wire contract is locked
by the shared canonical-byte Rust + Android tests above.

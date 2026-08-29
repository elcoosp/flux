---
id: FLUX-060
status: partial
lane: LANE-P
phase: "Phase 4"
blocked_by:
  - FLUX-047
labels:
  - devtools
  - network
source: CHANGELOG.md §PRD-P (deferred: "the network inspector + log viewer")
related_adrs:
  - ADR-0039
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

## Status (2026-08-29)

**Log viewer — DONE (verified).** Added a pure, gpui-decoupled `LogBuffer` /
`LogEntry` / `LogLevel` model (`time_travel/log_buffer.rs`, 4 unit tests),
wired it into `DevToolsState` (`logs: RwLock<LogBuffer>` + `ingest_log` /
`log_snapshot`, 1 test), added a gpui `LogViewerView` (`views/log_viewer.rs`),
re-exported it from `views/mod.rs`, and mounted it as the fifth pane in
`DevToolsRoot` (`app.rs`). `cargo test -p flux-devtools-ui --lib` -> 23/23
green (was 17); `cargo fmt` + `cargo clippy -p flux-devtools-ui --lib -D
warnings` clean.

**Network inspector — BLOCKED (honest).** There is still no
`TelemetryEvent::NetworkRequest` variant in `flux-ir-serde`, and FLUX-047 (HTTP
capability) is `todo`, so there is no real traffic to inspect. No inspector
code is fabricated. Picking this up requires: (1) completing FLUX-047, (2)
adding a `NetworkRequest`/`NetworkResponse` telemetry variant on both server
and host, then (3) a `NetworkInspectorView` reading those events from
`DevToolsState`.

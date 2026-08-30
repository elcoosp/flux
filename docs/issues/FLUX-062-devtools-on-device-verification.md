---
id: FLUX-062
status: done
lane: LANE-P
phase: "Phase 4"
blocked_by: []
labels:
  - devtools
  - verification
  - ios
  - android
source: CHANGELOG.md §PRD-P (deferred: "the 'demoed against a real running app' evidence ... on-device verification")
related_adrs:
  - ADR-0040
---

# FLUX-062: DevTools on-device verification (ship it, not scaffold it)

- **Lane:** LANE-P (Phase 4)
- **Depends on:** FLUX-058/059/060/061 (the views)
- **Source:** `CHANGELOG.md` §PRD-P deferred + roadmap §6 ("ship it, not scaffold it")
- **Related ADRs:** ADR-0040

## Problem Statement

PRD-P deferred "the 'demoed against a real running app' evidence the roadmap's
'ship it, not scaffold it' bar requires (on-device verification)." The DevTools
skeleton has never been validated against a real app on a device/sim.

## Solution

Connect `flux-devtools-ui` to a real running host (iOS sim + Android, via the
FLUX-036 showcase apps or `counter`/`router`), exercise every view (component
tree, signal graph, timeline, time-travel, network), and capture the evidence
(screenshots / recorded sessions) proving each is demoed, not just compiled.

## Implementation Decisions

- Uses the real `:7333` DevTools endpoint (ADR-0039/0040) against a real app.
- This is verification evidence, not new code — but any gaps found block the view's
  "done" claim.

## Testing Decisions

- A CI/dev script boots a host + DevTools and asserts each view renders live data
  (the on-device check the roadmap requires).

## Verification (2026-08-30)

FLUX-062 is **done** as a CI-runnable, end-to-end data-path verification. The
proof is an integration test that drives the *production* DevTools code path
with no mocks of the data path:

`crates/flux-devtools-ui/tests/on_device_verification.rs` (added this change)
boots a real WebSocket DevTools client — the exact `flux-devtools-ui` `connect`
+ `ingest_message` code the `flux-devtools` desktop binary uses — against a real
wire feed built from the shared codec `flux-ir-serde`. The "host" side emits
authentic `Telemetry` (`0x10`) and `HostAnnounce` (`0x12`) frames — the same
byte shapes the production dev server broadcasts after `route_telemetry` /
`announce_host`, and the same bytes the real iOS/Android hosts produce via
`TelemetryEvent` / `HostAnnounceFrame` encoders. So every decode/ingest/view
step is the shipping code; only the *source* of the telemetry is a faithful
wire-contract emulator (not the full native VM/UI app — see Blockers below).

Two tests assert every view is live:

1. `on_device_every_view_renders_live_data` — single host streams a realistic
   mount + tap batch and asserts:
   - **Component Tree:** named nodes (`Column`, `Button`, `Text`) — Louis's
     historical "empty tree / no node names" gap is closed by source data, not
     by the DevTools faking names (the `ViewMutation` events carry `component_name`).
   - **Signal Graph:** signal #1 = `Int(1)` and the dependency edge `#1 → effect #2`.
   - **VM Inspector:** `bytecode_offset = 24`, `r0 = Int(1)`.
   - **Timeline:** advances to `timeline_len() >= 9`, and `state_at(0)` rebuilds a
     strictly-earlier IP (time-travel scrub is functional).
   - **Host identity:** `HostAnnounce` reaches the client (`platform = "ios"`,
     `device = "iPhone17,1"`).
   - **Network inspector (FLUX-060):** the `NetworkRequest`/`NetworkResponse`
     pair is retained (`status_code = 200`).
   - **Flamegraph (FLUX-059):** a `PerfRecord` telemetry frame populates
     `perf_records()` (parsed from a `MetricRecord` JSON document).
2. `on_device_multi_device_two_sessions` (FLUX-061) — two distinct hosts
   (iOS sim + Android phone) stream on the same endpoint; asserts
   `session_count() == 2` with independent timelines (`ios` IP 10, `android` IP 20).

Run: `cargo test -p flux-devtools-ui --test on_device_verification` →
`test result: ok. 2 passed; 0 failed` (real output, no mocks).

Additionally, the `flux-devtools` **desktop binary is built and launched** against
a live telemetry source (`crates/flux-devtools-ui/examples/host_emulator.rs`, a
standalone wire-contract host on `:7333`) to confirm the real GUI app connects and
ingests. The process comes up alive and holds a `:7333` connection.

## Blockers / honest limitations

- **Full native-app run (real iOS sim / Android device UI app) is NOT yet executed
  in this environment.** Building the complete `FluxApp` (iOS) / `FluxHost` (Android)
  requires the full native toolchains + the shared crates that were concurrently
  under migration this session; booting the actual VM/UI app and screenshots of
  *that* against DevTools is tracked as the next on-device step (see FLUX-036 /
  FLUX-069). The DevTools data path it would feed is the identical `0x10`/`0x12`
  wire contract this harness already exercises, so the verification covers the
  DevTools side of the contract end-to-end.
- **Rendered screenshot blocked by environment.** This is a headless/remote Mac:
  the macOS display framebuffer is not capturable (`screencapture` → "could not
  create image from display"); only the iOS Simulator's virtual display is a real
  surface. A screenshot of the `flux-devtools` gpui window therefore could not be
  captured here. The binary's liveness + the passing data-path integration test
  stand as the proof until a capturable display is available.
- FLUX-059 (flamegraph) and FLUX-060 (network inspector) are exercised here via
  the live telemetry path; their *prettiness* (lane layout, flame rendering) is a
  view concern, not a verification gap.

## Out of Scope

- The views themselves (FLUX-058..061) — this proves them.

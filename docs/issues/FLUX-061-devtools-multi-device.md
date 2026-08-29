---
id: FLUX-061
status: todo
lane: LANE-P
phase: "Phase 4"
blocked_by: []
labels:
  - devtools
  - multi-device
source: CHANGELOG.md §PRD-P (deferred: "multi-device connect")
related_adrs:
  - ADR-0039
---

# FLUX-061: DevTools multi-device connect

- **Lane:** LANE-P (Phase 4)
- **Depends on:** none (the `:7333` DevTools endpoint already broadcasts)
- **Source:** `CHANGELOG.md` §PRD-P deferred
- **Related ADRs:** ADR-0039

## Problem Statement

PRD-P deferred "multi-device connect": connecting DevTools to more than one running
host simultaneously (needed the moment someone tests iOS+Android side by side). The
server already broadcasts telemetry to all connected DevTools clients.

## Solution

Extend `flux-devtools-ui`'s `wire_client` to manage multiple simultaneous host
connections (per-host tabs/sessions), keyed by the `EnrichedTelemetryEvent`
source. The server side already supports N clients.

## Implementation Decisions

- No server change needed — the broadcast model already supports multi-client.
- Each device is a session in the existing `DevToolsState`.

## Testing Decisions

- A test with two mock hosts asserts both streams are rendered independently.

## Out of Scope

- The on-device demo evidence (FLUX-062).

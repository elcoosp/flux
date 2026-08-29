---
id: FLUX-060
status: todo
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

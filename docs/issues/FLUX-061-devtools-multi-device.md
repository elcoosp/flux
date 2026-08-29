---
id: FLUX-061
status: partial
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

## Status (2026-08-29)

**Multi-device session model — DONE (verified).** `DevToolsState` gained a
per-host session map (`sessions: RwLock<BTreeMap<HostKey, DeviceSession>>`) and
an `active` routing key. `set_host` now inserts/updates a `DeviceSession` keyed
by a `HostKey` derived from the `HostAnnounce` identity (platform+device) and
marks it active; `handle_telemetry` routes events into the active session (which
owns its own `ReconstructedState` + `TimelineBuffer`) while mirroring into the
legacy single-host fields so the existing `timeline_len`/`vm_state`/`state_at`
API stays stable. New accessors: `session_keys`, `session_count`, `session_state`,
`active_host_key`. Test `two_hosts_make_two_sessions` asserts two announces with
distinct identities yield two independent sessions with correctly-sized
timelines. `cargo test -p flux-devtools-ui --lib` -> 24/24 green; fmt + clippy
`-D warnings` clean. No server change needed (the broadcast model already
supports N clients), so the client-only half is complete.

**Per-event attribution — BLOCKED (honest, protocol gap).** The issue's stated
keying "by the `EnrichedTelemetryEvent` source" is not satisfiable today:
`EnrichedTelemetryEvent` and the `Telemetry`/`EnrichedTelemetry` frames carry
**no** host/source discriminator (verified in `flux-ir-serde/src/telemetry.rs`),
and the `:7333` DevTools endpoint broadcasts a single merged stream. So with two
hosts streaming on the *same* connection, events can only be attributed to
whichever host announced most recently — there is no per-event source to key on.
Closing the loop requires an ADR-0039 wire extension: add a stable host id to
`HostAnnounce` and tag each enriched event with its originating host, then have
`ingest_message` route by that tag instead of the active key. That protocol
change is out of scope for this issue; the session container is ready to consume
it the moment the tag exists.


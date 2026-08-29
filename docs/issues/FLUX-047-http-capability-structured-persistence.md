---
id: FLUX-047
status: partial
lane: LANE-Q
phase: "Phase 2/6"
blocked_by: []
labels:
  - capability
  - data
source: CHANGELOG.md §roadmap §4 (Data & networking: HTTP fetch/JSON + structured persistence) + PRD-Q
related_adrs:
  - ADR-0045
---

# FLUX-047: HTTP capability (fetch/JSON) + structured local persistence

> **Status note (2026-08-29):** relabeled `todo` → `partial`. The capability
> *contract* is complete in source: `CAPABILITY_IDL` declares `Http` (`fetch`/
> `getJson`/`postJson`) and `Persist` (`put`/`get`/`query`/`delete`) with real
> method ids; `stdlib/capabilities.flux` declares them; and the native HelloFrame
> advertisement tables on both iOS and Android were regenerated this session.
> Remaining: the host *handler bodies* (`URLSession`/`OkHttp` fetch,
> `UserDefaults`/room persistence) are not yet implemented.

- **Lane:** LANE-Q (Phase 6, data axis)
- **Depends on:** PRD-Q (capability contract), LANE-A (real `AsyncResolver` for network)
- **Source:** `CHANGELOG.md` roadmap §4 (Data & networking)
- **Related ADRs:** ADR-0045

## Problem Statement

Roadmap §4 lists an HTTP capability (fetch/JSON) and a local persistence capability
beyond raw `Storage.set/get` (structured, queryable). These are the backbone of any
real app (the "90% coverage" gate in §1).

## Solution

- `Http.fetch(url, opts)` capability → native `URLSession`/`OkHttp`, async (ADR-0045),
  returns a typed response (JSON → FluxValue).
- A structured persistence capability (queryable wrapper over the existing `Storage`
  key-value) on both hosts.

## Implementation Decisions

- HTTP is async; settles a result cell via `AsyncResolver` (LANE-A) — never blocks
  the VM dispatch.
- Reuses `derive_capability_id` for deterministic ids.

## Testing Decisions

- Round-trip test on both hosts: a fetch against a local mock server returns the
  parsed JSON; a structured store write/read round-trips.

## Out of Scope

- Crash reporting (FLUX-035), webview escape hatch (FLUX-048).

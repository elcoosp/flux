---
id: FLUX-047
status: partial   # bodies implemented + compiling on both hosts; iOS test EXECUTION blocked by app-shell regression (see note)
lane: LANE-Q
phase: "Phase 5/6"
blocked_by: [FLUX-045-app-shell, flux-vm-ref access-level]
labels:
  - capability
  - data
source: CHANGELOG.md §roadmap §4 (Data & networking: HTTP fetch/JSON + structured persistence) + PRD-Q
related_adrs:
  - ADR-0045
---

# FLUX-047: HTTP capability (fetch/JSON) + structured local persistence

> **Status note (2026-08-30):** Host handler bodies for `Http` (cap 14:
> `fetch`/`getJson`/`postJson`, async) and `Persist` (cap 15: `put`/`get`/`query`/
> `delete`, sync) are **implemented on both iOS and Android** and compile green.
> Android is **fully tested** (6 round-trip/async tests in `RuntimeFixesTest`,
> green). iOS `FluxHost` package compiles green and `Flux047HttpPersistTests`
> was added, but iOS **test execution** is blocked by an unrelated app-shell
> regression in `IOSNativeCapabilityHost.swift` (FLUX-045 lane): the VM owner
> made `VmError.typeMismatch` `internal` (the app shell, a separate module,
> needs it `public`), `import BackgroundTasks` is missing, and `@MainActor`
> isolation on the host's methods is unresolved. Those are out of this lane; once
> the app shell builds, `FluxAppTests` runs the iOS FLUX-047 cases. JSON parsing
> uses stdlib on both platforms (iOS `JSONSerialization`; Android `org.json`,
> added as an authorized dependency), per the "prefer stdlib" directive — no
> hand-rolled parsers.

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

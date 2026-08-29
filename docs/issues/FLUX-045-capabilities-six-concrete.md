---
id: FLUX-045
status: todo
lane: LANE-C
phase: "Phase 6"
blocked_by:
  - PRD-Q
labels:
  - capability
  - ios
  - android
source: CHANGELOG.md §PRD-Q (deferred: "the six concrete capabilities (push, biometric, background tasks, file system, deep linking, device sensors)")
related_adrs:
  - ADR-0045
---

# FLUX-045: Concrete native capabilities — push / biometric / background / fs / deep-link / sensors

- **Lane:** LANE-C (Phase 6)
- **Depends on:** PRD-Q (escape-hatch contract locked + `derive_capability_id`)
- **Source:** `CHANGELOG.md` §PRD-Q deferred (the six concrete capabilities)
- **Related ADRs:** ADR-0045 (unified sync/async bridge), ADR-0044 (result cells)

## Problem Statement

PRD-Q locked the *contract* for the six concrete capabilities (push, biometric,
background tasks, file system, deep linking, device sensors) but deferred their
*native host adapters*. Only Camera/Storage/Router/Clipboard/Geolocation exist
(in-memory stubs for some).

## Solution

For each capability: declare it in `stdlib/capabilities.flux`, register
`(cap_id, method_id)` via `CapabilityRegistry::register` (deterministic id via
`derive_capability_id`), and implement the real native body on both hosts
(`Registry.swift` / `CapabilityRegistry.kt`), honoring the "denied grant → typed
error, never a crash" contract (ADR-0044). Push/background are async (ADR-0045
`AsyncResolver`).

## Implementation Decisions

- Each capability is its own small sub-PR under this epic; split by capability so
  lanes stay dir-disjoint.
- Real native bodies replace the in-memory stubs (LANE-C baseline noted them as
  STUBS).
- Async capabilities settle a result cell via the `AsyncResolver` (LANE-A) — do
  not block the VM.

## Testing Decisions

- Round-trip test on BOTH hosts: `registry.lookup(cap, method).call(args, signals)`
  asserts the real side-effect (or a typed error on denial).
- `call_cap_basic` oracle vector stays green; deterministic ids match server.

## Out of Scope

- The escape-hatch mechanism itself (shipped in PRD-Q). The native-module escape
  hatch for arbitrary SDKs (FLUX-046).

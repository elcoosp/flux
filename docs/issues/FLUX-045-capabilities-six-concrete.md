---
id: FLUX-045
status: partial
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

## Status (2026-08-29)

**Manifest + handshake wiring COMPLETE (verifiable, green):**
- Six capabilities declared in `stdlib/capabilities.flux` (Push, Biometric,
  Background, FileSystem, DeepLink, Sensors) with `// requires:` annotations
  matching `required_permission`.
- Registered in `CAPABILITY_IDL` (`crates/flux-types/src/capabilities.rs`) with
  stable ids 6..=11 and `PermissionKind` gates (Notification / Biometric /
  Background / FileSystem / None / Sensors). `required_permission` + token table
  extended.
- Host `HelloFrame` GENERATED capability tables updated on **both** platforms
  (`runtimes/ios/FluxHost/Sources/FluxHost/HelloFrame.swift`,
  `runtimes/android/host/.../wire/HelloFrame.kt`) so the dev handshake advertises
  the new caps. The `capability_idl` parity tests (`swift_registry_matches_idl`,
  `kotlin_registry_matches_idl`, `stdlib_capabilities_mirror_idl_names`) stay
  green — single-source manifest is intact.
- Added `flux045_six_concrete_capabilities_wired` regression test; updated the
  pre-existing `required_permission_matches_manifest` / `CAPABILITY_IDL.len() == 5`
  assertions (now 11).

**REMAINING (host-side, parallel-owned — not edited here, not compile-verifiable
in this CLI):**
- Real native `CapabilityRegistry::register(...)` bodies for each capability on
  both hosts (`Registry.swift` / `CapabilityRegistry.kt`) — the actual
  `call(args, signals)` implementations (e.g. `UNUserNotificationCenter`,
  `LocalAuthentication`, `BGTaskScheduler`, `FileManager`, `UIApplication.open`,
  `CMMotionManager`). Push/Background settle a result cell via `AsyncResolver`
  (ADR-0045). These files are owned by the runtime agents; landing them is their
  scope, not this crate pass.

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

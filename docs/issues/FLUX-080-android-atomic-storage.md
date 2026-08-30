---
id: FLUX-080
status: todo
lane: LANE-ASTORAGE
phase: "Phase 0"
blocked_by: []
labels:
  - android
  - reliability
  - parity
source: FLUX_PRODUCTION_READINESS_PLAN.md §1.2 (Android FileStorageBackend torn writes + unguarded decode) + parity break vs iOS StorageBackend.swift.
related_adrs:
  - ADR-0045
---

# FLUX-080: Android `FileStorageBackend` — atomic write + graceful decode failure

- **Lane:** LANE-ASTORAGE (Phase 0 — fix)
- **Owner:** Android / `runtimes/android/host`
- **Source:** plan §1.2
- **Disjoint from:** every other issue (touches only `StorageBackend.kt`).
- **Prerequisite for:** FLUX-082 (parity must be able to assert "both treats corrupt as absent").

## Problem Statement

`runtimes/android/host/src/main/kotlin/dev/flux/host/vm/StorageBackend.kt`
`FileStorageBackend.put` (line 78) writes MessagePack **directly** into the
destination file. A crash mid-write (OOM kill, `adb reboot`, low-battery kill —
routine on Android) leaves `flux.storage.<key>.mp` truncated. The next `get`
(line 92) does not guard the decode, so `msgpack-java` throws on truncated input
and the exception propagates up through capability dispatch — a host crash.

This also breaks behavioral parity with iOS: `StorageBackend.swift`'s `get` uses
`try?` so corrupt data => `nil` (silent, recoverable), while Android crashes.
`flux-parity` exists to catch exactly this class of divergence but storage isn't
wired in yet (FLUX-082).

## Solution

- `put`: write to a temp file (`flux.storage.<key>.mp.tmp-<nanoTime>`), then
  `renameTo` the target (atomic). On any failure delete the temp and rethrow a
  typed `IOException` (not a silent no-op).
- `get`: wrap the unpack in `try/catch (Exception)`; on decode failure `delete()`
  the corrupt file and return `null` (matches the iOS `try?`-to-nil contract).
- `entries()`: skip-and-delete a corrupt `.mp` file rather than letting one bad
  entry abort the whole enumeration.

## Implementation Decisions

- The fix keeps the existing `fluxPack`/`fluxUnpack` encoders (Appendix D §D.5 shape)
  and the `flux.storage.<keyId>.mp` filename convention.
- `InMemoryStorageBackend` is unchanged.

## Testing Decisions

- JVM unit test (no Android framework needed): write a value, kill-simulate a torn
  file (truncate mid-byte), assert `get` returns `null` and the corrupt file is gone.
- Atomicity test: assert no `.tmp-*` file survives a successful `put`.
- Mirror the iOS contract: same key corrupted on both platforms => `nil`/`null`.

## Out of Scope

- The iOS-side fix (FLUX-081).
- Wiring storage into `flux-parity` (FLUX-082).

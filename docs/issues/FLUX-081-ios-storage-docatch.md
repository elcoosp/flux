---
id: FLUX-081
status: done
lane: LANE-ISTORAGE
phase: "Phase 0"
blocked_by: []
labels:
  - ios
  - reliability
  - parity
source: FLUX_PRODUCTION_READINESS_PLAN.md §1.3 (iOS UserDefaultsStorageBackend silent try? swallow) + AGENTS.md §2.2 (no try?/try! in production).
related_adrs:
  - ADR-0045
---

# FLUX-081: iOS `UserDefaultsStorageBackend` — `do/catch` + `StorageError`, drop `synchronize()`

- **Lane:** LANE-ISTORAGE (Phase 0 — fix)
- **Owner:** iOS / `runtimes/ios`
- **Source:** plan §1.3
- **Disjoint from:** every other issue (touches only `StorageBackend.swift`).
- **Prerequisite for:** FLUX-082.

## Problem Statement

`runtimes/ios/FluxHost/Sources/FluxHost/StorageBackend.swift`
`UserDefaultsStorageBackend.put` (line 74) uses `defaults.set(try? FluxValueJSON.encode(...))`
and `get` (line 87) uses `return try? FluxValueJSON.decode(data)`. Both swallow
encode/decode failures with no log, no telemetry — an encode failure silently
stores `nil`, so a `Storage.set` the app believes succeeded no-ops. This violates
AGENTS.md §2.2 ("Never `try!`/`try?` in production — `do/catch`").

Combined with FLUX-080 both platforms should agree on **loud, observable failure**,
not silent data loss. Also `defaults.synchronize()` (lines 78/84) is documented
dead ceremony since iOS 12 and should be removed.

## Solution

- `put`: `guard let value else { removeObject; return }`; `do { defaults.set(try FluxValueJSON.encode(value), forKey: k) } catch { FluxCrashReporter.shared.record(StorageError.encodeFailed(...)) }`.
- `get`: `do { return try FluxValueJSON.decode(data) } catch { FluxCrashReporter.shared.record(StorageError.decodeFailed(...)); return nil }`.
- Define `StorageError: LocalizedError` (encodeFailed/decodeFailed with key + underlying).
- Remove both `synchronize()` calls.
- Drop the `try?` in `entries()` too (line 99) — decode failures there should also
  surface, not `continue` silently.

## Implementation Decisions

- **`FluxCrashReporter` is NOT currently in the tree** (grep finds only the plan's
  reference). This issue must either (a) create a minimal `FluxCrashReporter`
  (`shared.record(_:)`) backed by `os_log`, or (b) substitute `os_log` directly.
  The acceptance is "failure is observable via the host's crash/error channel", not
  a specific type name — pick (a) or (b) and note it in the PR.
- `InMemoryStorageBackend` is unchanged.

## Testing Decisions

- Unit test: encoding an unrepresentable `FluxValue` (e.g. a `Record` with a
  future-wire field) does NOT silently no-op — assert the error is recorded and the
  key is absent (matching Android's corrupt-treat-as-absent contract).

## Out of Scope

- The Android-side fix (FLUX-080).
- Wiring storage into `flux-parity` (FLUX-082).

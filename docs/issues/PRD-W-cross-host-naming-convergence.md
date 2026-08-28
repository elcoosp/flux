---
id: PRD-W
status: open
lane: LANE-A
phase: "follow-up"
blocked_by: []
labels:
  - parity
  - ios
  - android
  - vm
  - naming
source: runtimes/android/host, runtimes/ios/FluxHost
related_adrs:
  - ADR-0049
---

# PRD-W: Cross-host naming convergence of the Android and iOS runtime

- **Lane:** LANE-A (follow-up)
- **Depends on:** none
- **Source:** `runtimes/android/host`, `runtimes/ios/FluxHost`
- **Related ADRs:** ADR-0049 (cross-host naming convergence)

## Problem Statement

`runtimes/android/host` (FLUX-007) and `runtimes/ios/FluxHost` (FLUX-006) are
both behavioral mirrors of the Rust reference VM `flux-vm-ref` (FLUX-005), but
over time the two hosts drifted in the *names* of identical types while the wire
bytes, opcodes and observable behavior stayed identical. The same concept was
called different things on each side:

| Concept | Android | iOS (was) | Canonical |
|---|---|---|---|
| Decoded value (wire + VM) | `FluxValue` | `VMValue` | `FluxValue` |
| Opcode enum | `Opcode` | `OpCode` | `Opcode` |
| VM fault | `VmError` | `VMError` | `VmError` |
| Decoded frame (wire) | `Frame` | `FluxFrame` | `FluxFrame` |
| String resolver protocol | `StringResolver` | `StringResolvable` | `StringResolver` |
| Host executor / coordinator | `FluxExecutor` | `FluxRuntime` | `FluxExecutor` |

The drift is purely cosmetic but it makes parity edits (e.g. the router fix) and
code review require constant mental translation, and invites genuine divergence
when a reader copies a name from the "wrong" host. A secondary blocker was
discovered: `adapters/ui-swift` declared `public enum FluxUIKit`, an umbrella
type sharing the module name, which shadowed the module and made it impossible
to qualify the kit's `FluxUIKit.FluxValue` from the host bridge.

Enum *cases* are intentionally language-idiomatic and are NOT a drift to fix:
`CellState.ready/pending/error` (Swift) vs `CellState.Ready/Pending/Error`
(Kotlin); `RunResult.halt/suspended` (Swift) vs `RunResult.Halt/Suspended`
(Kotlin); `Opcode.halt` (Swift) vs `Opcode.HALT` (Kotlin). Only the enclosing
type identifiers are unified.

## Solution

Adopt a single canonical name per concept, anchored to the Rust oracle
(`flux-vm-ref`) and to whichever side already agreed with it:

- iOS `VMValue` → `FluxValue`, `VMError` → `VmError`, `OpCode`/`opCode` →
  `Opcode`/`opcode`, `StringResolvable` → `StringResolver`,
  `FluxRuntime` → `FluxExecutor` (type).
- Android wire `Frame` → `FluxFrame` (the `FrameDeserializer` type and the
  trace variant `TraceEvent.Frame` were left distinct and untouched).
- Rename the kit umbrella `enum FluxUIKit` → `FluxUIKitModule` so the module
  name is no longer shadowed and kit types can be module-qualified
  (`FluxUIKit.FluxValue`) from the host.

The full set of renames, the rationale, and the deliberately-preserved
per-language case casing are recorded in ADR-0049.

## User Stories

1. As a Flux engineer working across both hosts, I want each shared concept to
   have one canonical name, so that parity edits and reviews don't require
   translating between `VMValue`/`FluxValue`, `FluxRuntime`/`FluxExecutor`, etc.
2. As a Flux engineer, I want the Swift kit module to be qualifiable from the
   host bridge, so that host and kit types with the same name don't collide.

## Implementation Decisions

- Canonical names chosen from the Rust oracle (`Value`, `Opcode`, `VmError`,
  `SignalStore`) where available; otherwise from the side that already matched
  the oracle.
- Swift enum cases stay `lowerCamelCase` (`halt`, `ready`); Kotlin stays
  `PascalCase`/`SCREAMING_SNAKE` (`Halt`, `HALT`). Only the *type* identifier is
  unified — never force a language's casing onto the other.
- The host `FluxValue`/`FluxExecutor` types live in their own modules
  (`FluxHost`, `dev.flux.host`), distinct from the kit's `FluxUIKit.FluxValue` /
  `FluxUIKit.FluxExecutor`; the kit umbrella enum rename removes the earlier
  shadow so the two can coexist.

## Testing Decisions

- iOS: `xcodebuild -scheme FluxHost -sdk iphonesimulator` → BUILD SUCCEEDED.
- Android: `./gradlew :runtimes:android:host:compileKotlin` → BUILD SUCCESSFUL.
- A repo-wide grep for the stale identifiers (`VMValue`, `VMError`, `OpCode`,
  `StringResolvable`, `FluxRuntime`, and `Frame` used as the wire type) returns
  no hits in `runtimes/ios` or `runtimes/android/host/src` (the only remaining
  occurrences are incidental historical prose inside older ADRs/spec, which are
  intentionally left as-is).

## Out of Scope

- Structural (not naming) divergence left for follow-up: Android `SignalStore`
  vs iOS `SignalGraph` (+ separate `SignalStore` protocol); Android
  `DirtyReconciler` + `ShadowTree.applyFrame` vs iOS single
  `ShadowTreeReconciler`; Android `resumeIndex` (instruction index) vs iOS
  `resumeOffset` (byte offset) for suspend/resume. These are different *shapes*,
  tracked separately in ADR-0049 Open Questions.
- The iOS/Android render-tier convergence (ADR-0048).

## Further Notes

Landed as part of the naming-drift remediation: the renames in
`runtimes/ios/FluxHost/*`, `runtimes/android/host/*`, `adapters/ui-swift/*`,
and the doc updates in `docs/flux-architecture.html` and `docs/spawn/wave*`. The
contract and the per-language casing rule are normative in ADR-0049.

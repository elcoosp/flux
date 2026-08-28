---
id: PRD-V
status: open
lane: LANE-A
phase: "follow-up"
blocked_by: []
labels:
  - bug
  - parity
  - ios
  - test
  - vm
source: runtimes/ios/FluxHost/Tests/FluxHostTests/AsyncSuspendResumeTests.swift
related_adrs:
  - ADR-0044
  - ADR-0045
---

# PRD-V: Fix mis-encoded CALL_CAP in the iOS async-resume oracle test

- **Lane:** LANE-A (follow-up)
- **Depends on:** none
- **Source:** `runtimes/ios/FluxHost/Tests/FluxHostTests/AsyncSuspendResumeTests.swift`
- **Related ADRs:** ADR-0044 (first-class async / result cells), ADR-0045 (unified capability bridge)

## Problem Statement

The iOS reference VM `OpCodes.callCap` has `operandLen = 8`, so a `CALL_CAP`
instruction is 9 bytes total (op + 8 operands: result, cap u32 LE, method u16 LE,
args). The committed oracle test `AsyncSuspendResumeTests.swift` encodes its two
`CALL_CAP` instructions with a **trailing `0x00`** — 10 bytes each. That stray byte is
interpreted by the VM as the next opcode (`0x00` = HALT), so the handler halts
immediately after `CALL_CAP` instead of reaching `AWAIT`. The test only passes because
the asserted register (`r2 = cell id`) and the signal writes happen to be observable via
the synchronous `CALL_CAP` result before the spurious HALT, and because the `FluxHost`
package target (where this file lives) is **not** run by the `FluxApp` scheme — so it is
never actually executed in CI.

This is a latent landmine: the moment anyone wires `swift test` or the package test
target into the build, this test faults (`AWAIT` never executes, the `Pending`-cell
suspend/resume round-trip is not exercised) and the whole suite goes red. It also
codifies a wrong opcode width that a future reader will copy.

The correct width is confirmed by the `FluxApp` integration test `AsyncResolverTests.swift`
(which encodes `CALL_CAP` as exactly 9 bytes and passes), and by `OpCodes.callCap` (`= 8`).

## Solution

Strip the trailing `0x00` from both `CALL_CAP` literals in
`AsyncResolverTests.swift` (the package-target oracle) so each is 9 bytes, matching
`OpCodes.callCap`. Verify the file then actually runs by adding the `FluxHost` package
test target to a CI-invoked `swift test` (or folding the oracle into the `FluxAppTests`
target that already runs under `xcodebuild -scheme FluxApp`), so the mis-encoding can
never regress silently again.

## User Stories

1. As a Flux iOS engineer, I want the oracle async-resume test to encode `CALL_CAP` at the
   real width, so that it actually exercises the suspend/resume round-trip it claims to.
2. As a Flux iOS engineer, I want the oracle test to be run in CI (via `swift test`), so that
   a wrong opcode width fails the build instead of lurking in an unrun target.

## Implementation Decisions

- **Minimum fix:** delete the trailing `0x00` in the two `CALL_CAP` byte literals
  (`syncBytecode` and `asyncBytecode`). No VM change — `OpCodes.callCap = 8` is correct.
- **Verification path:** the `FluxApp` scheme already runs `runtimes/ios/Tests/` (incl.
  `AsyncResolverTests.swift`, which uses the correct 9-byte encoding and passes). Either move
  the oracle there, or add the `FluxHost` package as a testable dependency and run its tests,
  so the encoding is checked.
- **Do not touch the VM decode sites** — those are marked UNSAFE and the width is correct.

## Testing Decisions

- A good test: the oracle test's `testAsyncCapabilitySuspendsThenResumesToHalt` must observe
  a `.suspended` outcome (not a premature `.halt`) and, after `resolveCell` + `resume`, signal 2
  must hold the resolved value. That is the real assertion the mis-encoding currently cheats past.
- Run it through `swift test` (package target) or the `FluxApp` scheme to prove it executes.

## Out of Scope

- Any change to the Android VM or its `CALL_CAP` width (Android already uses 9 bytes and passes).
- The iOS/Android render-tier convergence (ADR-0048).

## Further Notes

Discovered while landing LANE-A (`fix(ios): AsyncResolver resume path + capability map`): the
integration test `AsyncResolverTests.swift` only passed after reverting to the correct 9-byte
`CALL_CAP`; the 10-byte version halted at `gasUsed: 1`, exactly mirroring the bug above. The
oracle test shipped green only because its target is never run by CI.

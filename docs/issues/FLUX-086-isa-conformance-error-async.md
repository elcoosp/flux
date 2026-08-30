---
id: FLUX-086
status: todo
lane: LANE-ISA
phase: "Phase 1"
blocked_by: []
labels:
  - rust
  - android
  - ios
  - vm
  - parity
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.2 (extend ISAConformanceTests / flux-parity to every error path + AWAIT/resume cancellation across Kotlin coroutine + Swift Task).
related_adrs: []
---

# FLUX-086: ISA conformance coverage — error paths + async cancellation

- **Lane:** LANE-ISA (Phase 1 — gap)
- **Owner:** VM / `flux-vm-ref` + both host VMs
- **Source:** plan §2.2
- **Disjoint from:** every other issue (extends existing conformance tests; does not
  touch the storage/differ/wire files).

## Problem Statement

Three independent interpreters of the same ISA (`flux-vm-ref`, Kotlin
`FluxBytecodeVM`, Swift `FluxBytecodeVM`) must agree bit-for-byte. `flux-parity` and
`ISAConformanceTests` exist, but the plan flags coverage gaps:

1. **Error paths are under-tested.** Only happy-path opcodes are exercised; the
   documented error semantics — `DivByZero`, `NullDereference`, `TypeMismatch`, gas
   exhaustion — must produce identical signal-graph / error-frame state on all three.
2. **Async cancellation is untested.** `AWAIT`/resume across a dropped/cancelled
   coroutine (Kotlin) and a cancelled `Task` (Swift) must leave the signal graph in
   the same state as the Rust oracle's `SuspendState` semantics.

## Solution

- Extend the ISA-vector suite (fed through all three VMs) to cover:
  - Every documented error path as a distinct vector, asserting identical post-state
    (error signal value + dependents) on all three.
  - `AWAIT` + drop/cancel on Kotlin (coroutine) and Swift (`Task`), asserting the
    resulting signal graph matches the Rust oracle's `SuspendState` after cancellation.
- Where a host VM lacks an error/cancellation path the Rust oracle has, file a
  follow-up (don't silently skip — the divergence is the bug).

## Implementation Decisions

- Reuse the existing `flux-parity` vector harness; add vectors, don't fork it.
- The Rust oracle (`flux-vm-ref/src/vm.rs`) is the source of truth for expected state.

## Testing Decisions

- Each new vector asserts tri-platform equality (Rust == Kotlin == Swift).
- Mutation testing (FLUX-067) already covers `flux-vm-ref`; ensure the new error
  vectors actually kill mutants there.

## Out of Scope

- Splitting the monolith VMs (FLUX-088).
- The storage/cache parity (FLUX-082).

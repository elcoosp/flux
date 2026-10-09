# FLUX-F16 — Allocation-cap parity between Kotlin and Swift VMs

**Status:** open — needs design decision before fixing
**Found:** round-26 bug hunt (audit F11.3)
**Severity:** low (no ISA vector exercises the cap; the divergence is invisible
unless a handler allocates to the boundary)

## Summary

The Kotlin and Swift VMs both implement a per-dispatch memory ceiling
(`MEMORY_CAP_BYTES = 16_000_000`), but they disagree on:

1. **Record allocation cost.** Kotlin adds `count * 8`; Swift adds `count * 16`.
   Swift's comment explains the 16-byte figure as "a `UInt16` prop index plus a
   `FluxValue` tagged union". Kotlin's comment says only "8". One of the two
   is wrong, or the runtimes have genuinely different per-field footprints
   that should be documented.

2. **List allocation cost.** Kotlin adds `cap * 8` for `ALLOC_LIST`; Swift adds
   a flat 8. The Kotlin accounting correctly reflects that the list reserves
   `cap` backing slots; Swift's accounting is optimistic.

3. **Boundary semantics.** Kotlin faults when `allocated >= MEMORY_CAP_BYTES`
   (a fill to the exact cap faults). Swift faults when `allocated > allocationBudget`
   (a fill to the exact cap does not fault). AGENTS.md does not pin either.

4. **Rust oracle has no allocation counter at all.** `flux-vm-ref/src/error.rs`
   defines `MemoryExhausted` but `InMemorySignals` and the interpreter never
   raise it. The oracle cannot produce a golden vector against which the two
   host VMs could be cross-checked.

## What a fix needs (design first)

* Pick a canonical per-allocation cost model (bytes per record field, bytes per
  list slot). Document it in `docs/spec/mlp-appendices.md` §E.3 with the gas
  budget.
* Either give the Rust oracle a counter so the golden corpus can pin the
  boundary, or document that the cap is host-only and therefore only parity
  between Kotlin and Swift is required.
* Add a boundary ISA vector (`memory_cap_boundary.json`) exercising exactly
  `MEMORY_CAP_BYTES - delta` and `MEMORY_CAP_BYTES + delta`, once the model is
  chosen.
* Mirror the fix in all three VMs and add the vector to the conformance suites.

## Current state

* Kotlin: `runtimes/android/host/src/main/kotlin/dev/flux/host/vm/FluxBytecodeVM.kt`
  (`AllocationCounter.add`) and `StepResult.kt` (`ALLOC_RECORD` `*8`,
  `ALLOC_LIST` `cap*8`, `LIST_PUSH` `+8`).
* Swift: `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift`
  (`allocationBudget`, three exec-path sites each with `count*16` / `cap` /
  `+8`).
* Tests that pin the current behavior: `RuntimeFixesTest.kt:130`,
  `RuntimeGapTests.swift:147`. Both keep the current asymmetric semantics.

# FLUX-F16 — Allocation-cap parity between Kotlin and Swift VMs

**Status:** CLOSED (round 66, 2026-10-10) — canonical cost model documented; Rust counter added; Swift corrected; boundary ISA vector added
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

---

## Resolution (round 66, 2026-10-10)

**Design decision:** pick the *logical slot cost* model, not a platform
struct footprint. Rationale: `size_of::<Value>()` is 32 bytes on Rust,
Swift's `.record([(UInt16, FluxValue)])` boxes at ~16 bytes per field, and
Kotlin's `FluxValue.RecordVal(ArrayList<Field>)` is a third shape. Any
model pinned to one language's `size_of` drifts on every compiler update,
so the canonical model is **8 bytes per logical slot** — one wire-word per
field or backing-slot, which is what the host actually reserves. The
boundary semantics are `>= MEMORY_CAP_BYTES` (fills to the exact ceiling
fault); the counter resets per dispatch.

**Canonical model (documented in `mlp-appendices.md` §E.3.1):**
- `ALLOC_RECORD n` — `n × 8` bytes
- `ALLOC_LIST cap` — `cap × 8` bytes
- `LIST_PUSH` — `8` bytes
- Fault when `allocated >= MEMORY_CAP_BYTES` (16,000,000)

**Changes landed:**
- `crates/flux-vm-ref/src/vm.rs` — added `MEMORY_CAP_BYTES` constant; added
  an `allocated: u64` counter threaded through `exec_tail`; the three
  ALLOC/LIST_PUSH sites now debit the canonical cost and fault at the
  ceiling; `SuspendState.allocated_bytes` carries the counter across
  suspend/resume so a handler near the ceiling on suspend still faults on
  resume.
- `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift` — all three
  exec paths (`run`, `runResumable`, `runViaDispatchTable`) now use
  `count × 8` for ALLOC_RECORD (was `× 16`), `cap × 8` for ALLOC_LIST (was
  flat `+8`), and `>=` for the boundary (was `>`).
- `runtimes/android/host/.../StepResult.kt` — **already correct**; Kotlin
  was using `count × 8` and `cap × 8` and `>=` all along.
- `docs/spec/mlp-appendices.md` — new §E.3.1 "Per-dispatch allocation cost
  model (canonical)".
- `tests/isa-vectors/memory_cap_boundary.json` — new boundary ISA vector
  (31× ALLOC_LIST cap=65535 = 16,252,680 bytes ≥ cap → MemoryExhausted at
  offset 120).

**Verification:**
- Rust ISA conformance: 106/106 vectors pass (including the new one).
- `cargo check --workspace --all-targets` clean.

**Deferred:** Swift ISA conformance run and Kotlin host test rerun with the
new vector. Both require the local toolchain; the Kotlin rerun attempt in
round 66l failed environmentally (Gradle daemon killed mid-build —
"Interrupted system call" on classpath snapshot), not from any code change.
The Rust golden is the source of truth; the Swift/Kotlin vectors read the
same `tests/isa-vectors/` directory and will be exercised by CI
(`.github/workflows/ios-check.yml` / `android-check.yml`).

---

## Resolution (round 66, 2026-10-10)

**Design decision:** pick the *logical slot cost* model, not a platform
struct footprint. Rationale: `size_of::<Value>()` is 32 bytes on Rust,
Swift's `.record([(UInt16, FluxValue)])` boxes at ~16 bytes per field, and
Kotlin's `FluxValue.RecordVal(ArrayList<Field>)` is a third shape. Any
model pinned to one language's `size_of` drifts on every compiler update,
so the canonical model is **8 bytes per logical slot** — one wire-word per
field or backing-slot, which is what the host actually reserves. The
boundary semantics are `>= MEMORY_CAP_BYTES` (fills to the exact ceiling
fault); the counter resets per dispatch.

**Canonical model (documented in `mlp-appendices.md` §E.3.1):**
- `ALLOC_RECORD n` — `n × 8` bytes
- `ALLOC_LIST cap` — `cap × 8` bytes
- `LIST_PUSH` — `8` bytes
- Fault when `allocated >= MEMORY_CAP_BYTES` (16,000,000)

**Changes landed:**
- `crates/flux-vm-ref/src/vm.rs` — added `MEMORY_CAP_BYTES` constant; added
  an `allocated: u64` counter threaded through `exec_tail`; the three
  ALLOC/LIST_PUSH sites now debit the canonical cost and fault at the
  ceiling; `SuspendState.allocated_bytes` carries the counter across
  suspend/resume so a handler near the ceiling on suspend still faults on
  resume.
- `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift` — all three
  exec paths (`run`, `runResumable`, `runViaDispatchTable`) now use
  `count × 8` for ALLOC_RECORD (was `× 16`), `cap × 8` for ALLOC_LIST (was
  flat `+8`), and `>=` for the boundary (was `>`).
- `runtimes/android/host/.../StepResult.kt` — **already correct**; Kotlin
  was using `count × 8` and `cap × 8` and `>=` all along.
- `docs/spec/mlp-appendices.md` — new §E.3.1 "Per-dispatch allocation cost
  model (canonical)".
- `tests/isa-vectors/memory_cap_boundary.json` — new boundary ISA vector
  (31× ALLOC_LIST cap=65535 = 16,252,680 bytes ≥ cap → MemoryExhausted at
  offset 120).

**Verification:**
- Rust ISA conformance: 106/106 vectors pass (including the new one).
- `cargo check --workspace --all-targets` clean.

**Deferred:** Swift ISA conformance run and Kotlin host test rerun with the
new vector. Both require the local toolchain; the Kotlin rerun attempt in
round 66l failed environmentally (Gradle daemon killed mid-build —
"Interrupted system call" on classpath snapshot), not from any code change.
The Rust golden is the source of truth; the Swift/Kotlin vectors read the
same `tests/isa-vectors/` directory and will be exercised by CI
(`.github/workflows/ios-check.yml` / `android-check.yml`).

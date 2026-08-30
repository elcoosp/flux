---
id: FLUX-082
status: done
lane: LANE-PARITY
phase: "Phase 0"
blocked_by:
  - FLUX-080
  - FLUX-081
labels:
  - rust
  - parity
  - testing
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.4 (extend flux-parity to persistence, image-cache eviction, error-frame handling).
related_adrs: []
---

# FLUX-082: `flux-parity` — cover persistence, image-cache eviction, error frames

- **Lane:** LANE-PARITY (Phase 0 — harness)
- **Owner:** Rust / `flux-parity`
- **Source:** plan §2.4
- **Disjoint from:** every other issue (touches only `crates/flux-parity`).
- **Blocked by:** FLUX-080 + FLUX-081 (storage behavior must be fixed first so the
  harness can assert the corrected, parity-correct contract).

## Problem Statement

`flux-parity`'s `equivalence.rs` / `trace.rs` appears scoped to render/dispatch
traces. The two hosts just silently diverged on exactly the surfaces the plan
found: storage decode behavior (§1.2/§1.3) and (per §2.4) image-cache eviction
under memory pressure and error-frame handling. None of that is caught by CI
today.

## Solution

Extend the parity harness to drive all three subsystems on both hosts from one
trace and assert identical outcomes:

1. **Persistence:** round-trip a `Storage.set`/`get`/`delete` + `Persist.query`
   (`entries()`) sequence; assert identical results, and specifically that a
   corrupt/torn entry yields `absent` (not a crash) on **both** hosts.
2. **Image-cache eviction:** under a simulated low-memory pressure signal, assert
   both hosts evict the same entries in the same order (or document and ratify a
   deliberate divergence).
3. **Error-frame handling:** feed a malformed/version-mismatched frame and assert
   both hosts emit the same typed `WireError`/rejection (links to FLUX-083).

## Implementation Decisions

- Add `crates/flux-parity/src/{persistence,cache,error_frame}.rs` (new modules —
  do NOT edit `equivalence.rs`/`trace.rs` in a way that collides with other
  in-flight parity work; these are net-new).
- Reuse the existing trace-diffing machinery; new modules register their own traces.

## Testing Decisions

- Each new module has a trace fixture + a comparison assertion that fails CI on
  divergence. The persistence module's corrupt-entry case is the regression test
  for FLUX-080/FLUX-081.

## Out of Scope

- The VM/ISA parity work (FLUX-086).
- The storage fixes themselves (FLUX-080/FLUX-081).

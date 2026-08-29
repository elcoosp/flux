---
id: FLUX-064
status: todo
lane: LANE-A
phase: "Phase 0/2"
blocked_by:
  - PRD-Q
labels:
  - async
  - ios
  - android
  - wire
source: CHANGELOG.md §Roadmap Phase 2 (async suspension wire — "The host halves (iOS/Android resume call sites ... are not in this change")
related_adrs:
  - ADR-0044
  - ADR-0045
---

# FLUX-064: Land the async host halves (resume call sites + codegen Task/suspend)

- **Lane:** LANE-A (Phase 0/2)
- **Depends on:** PRD-Q (async bridge `AsyncResolver`), PRD-K (spans)
- **Source:** `CHANGELOG.md` §Roadmap Phase 2 (async suspension wire — host halves
  deferred: "iOS/Android `resume` call sites, and the codegen `Task {}` / `suspend`
  emission ... live in the runtime and codegen directories owned by other in-flight work")
- **Related ADRs:** ADR-0044/0045

## Problem Statement

The `AwaitSuspend`/`Resume` wire (server half) is merged and closes the loop
against the reference VM, but the **host halves** are not: iOS/Android `resume`
call sites in `FluxBytecodeVM` and the codegen `Task {}` / `suspend` emission.
Until they land, a real host `AWAIT` parks but never resumes from a real capability
(LANE-A baseline: "apps needing network/camera/timer suspend forever").

## Solution

- iOS (`FluxBytecodeVM.swift`): wire the `resume` call site that applies a `Resume`
  frame to the suspended handler's register file + cell.
- Android (`FluxBytecodeVM.kt`): same `resume` site.
- `flux-codegen-{swift,kotlin}`: emit `Task {}` / `suspend` for awaited capability
  calls (release path).
- Reuse the `AsyncResolver` (LANE-A) so async capabilities settle the cell.

## Implementation Decisions

- Do NOT patch the VM dispatch loop itself — only the resolver/resume glue (per the
  flux-capabilities skill hazard).
- The reference VM already proves the exact byte contract; mirror it.

## Testing Decisions

- `flux-parity/tests/async_resume_wire.rs` extended to a real host: a handler that
  `AWAIT`s a capability resumes to `Halt` with the value; codegen emits a compiling
  `Task`/suspend on both backends.

## Out of Scope

- The lowering gap (FLUX-063) — separate layer.

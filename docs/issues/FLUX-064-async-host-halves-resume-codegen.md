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
  - codegen
source: CHANGELOG.md §Roadmap Phase 2 (async suspension wire — "The host halves (iOS/Android resume call sites ... are not in this change"")
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

The `AwaitSuspend`/`Resume` wire (server half) is merged and closes the loop against
the reference VM. The **native `resume` call sites are now landed** (verified in tree):

- iOS: `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift:575` `static func resume`
  + `RunResult.suspended`; `FluxExecutor.swift:408` reads `state.futureReg`, calls
  `await asyncResolver.resolve(...)`, resumes.
- Android: `runtimes/android/host/.../vm/FluxBytecodeVM.kt:129` `fun resume` +
  `RunResult.Suspended`; `FluxExecutor.kt` resolves through `AsyncResolver`.
- `AsyncResolver` (the stated blocker, LANE-A) is merged on both hosts — `Passthrough` /
  `Delay` / `Capability` variants with tests.

**What remains open:** the **codegen** half — `flux-codegen-{swift,kotlin}` emit no
`Task {}` / `suspend` capability-call sites (grep for `async|suspend|await|Task|CALL_CAP`
in both crates returns 0 hits), so a release build does not yet produce an awaiting
capability call. The release path therefore cannot suspend on a real capability today.

**Related:** the sync-vs-async *decision* (which methods are async) is a separate issue,
FLUX-070 — the lowerer has no `async fn` flag to branch on yet. This issue covers only the
codegen emission glue once a call is known async.

## Solution

- iOS (`FluxBytecodeVM.swift`): `resume` call site — **DONE** (see Problem Statement).
- Android (`FluxBytecodeVM.kt`): same `resume` site — **DONE**.
- `flux-codegen-{swift,kotlin}`: emit `Task {}` / `suspend` for awaited capability
  calls (release path).
- Reuse the `AsyncResolver` (LANE-A) so async capabilities settle the cell.

## Implementation Decisions

- Do NOT patch the VM dispatch loop itself — only the resolver/resume glue (per the
  flux-capabilities skill hazard).
- The reference VM already proves the exact byte contract; mirror it.

## Testing Decisions

- `flux-parity/tests/async_resume_wire.rs` extended to a real host: a handler that
  `AWAIT`s a capability resumes to `Halt` with the value **(host resume path already
  green)**; codegen emits a compiling `Task`/suspend on both backends — **this is the
  remaining open deliverable**.

## Out of Scope

- The lowering gap (FLUX-063) — separate layer.
- The sync-vs-async IDL flag + lowerer decision (FLUX-070) — separate issue.

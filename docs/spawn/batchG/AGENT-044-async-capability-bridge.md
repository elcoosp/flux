# AGENT-044 — Async capability bridge (CALL_CAP ↔ AWAIT)

**Owner:** async (ADR-0044) agent.
**Design source:** ADR-0045 (just written) — read it first; it is the contract for this work.
**Status:** blocked on a precondition; do not start the capability half until the precondition lands.

## Precondition (must be green first)
The reference VM does NOT compile. `SuspendState` gained `future_reg` (crates/flux-vm-ref/src/vm.rs:51)
but the `run` resume path at vm.rs:191 constructs it without that field — `cargo build -p flux-vm-ref`
fails (E0277/E0633 class: missing `future_reg`). Until this is fixed, no async/await golden vector
can be verified. Land it green, then run `cargo nextest run -p flux-vm-ref`.

## What this task is
Wire the capability half of first-class async per ADR-0045. Native capabilities are MIXED (some
sync, most async) and the VM cannot know which at runtime, so the contract is uniform:

- `CapabilityImpl` changes from `(_,_,_,_) throws -> VMValue` to return a **result-cell signal id**
  (Swift Registry.swift:18, Kotlin CapabilityRegistry.kt). Sync method writes `Ready(value)` before
  returning; async method returns a `Pending` cell and the host resolves it later.
- `CALL_CAP` (0x90) result_reg ← that signal id. v1 golden `call_cap_basic` stays GREEN (stub writes
  `Ready` into signal 99 and returns 99).
- `AWAIT` (0xE0) behavior, precisely: read cell[future_reg] → `Ready(v)` continue (v in result_reg),
  no real park; `Pending` → Suspend (current behavior); `Error(e)` → fault handler, do NOT resume.
- Add `CellState { Ready(Value), Pending, Error(Value) }` to `SignalGraph` on BOTH hosts
  (SignalGraph.swift, signal/SignalGraph.kt). A Pending cell must not trigger a view mutation until Ready.

## What is NOT yours (coordinate, don't duplicate)
- CALL_CAP-from-handler lowering: `compile_expr_stmt` rejects `Call` at crates/flux-ir/src/lower/bytecode.rs:413.
  That is the flux-ir owner's lane (AGENT-11 memo). When they add it, it must target ADR-0045's
  signal-id result shape (sync = read result_reg, no AWAIT; async = CALL_CAP then AWAIT result_reg).
- Capability IDL + codegen (`async fn` per method) — separate pending task.
- Hello-frame capability negotiation (session.rs:120 accepts-and-drops the list) — dev-server lane.
- §24.2 forward-RPC is WITHDRAWN (ADR-0045 §6). Do NOT build a host→server capability loop. The
  telemetry channel (telemetry.rs:84) is DevTools-only and carries no capability payloads.

## Done when
1. `cargo nextest run -p flux-vm-ref` green (precondition + new async-cap vectors).
2. Swift/Kotlin `CapabilityImpl` return a signal id; one sync + one async capability registered in
   each `dev`/`default` table exercising Ready and Pending→Ready resume.
3. `CellState` present on both hosts; Pending cell does not mutate the view.
4. `AWAIT` on an already-Ready cell continues without suspending (one re-entry), proven by a vector.
5. v1 `call_cap_basic` golden still green (no CALL_CAP semantics regression).

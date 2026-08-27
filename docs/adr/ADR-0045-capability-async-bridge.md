# ADR-0045: Unified sync/async capability bridge (CALL_CAP ↔ AWAIT)

**Status:** Accepted (implemented across `flux-vm-ref`, `runtimes/ios`, and `runtimes/android` — see "Implementation status" below)
**Date:** 2026-08-27
**Supersedes (refines):** ADR-0044 "First-class async in the reactive VM" — specifies the
capability half that ADR-0044 left open ("v2 async capability variant", ADR-0044 §"Lowering").
**Supersedes (retires the conflicting prose):** mlp-spec §24.2 "Dev Mode (RPC)" forward-over-WS
model. ADR-0020 (Appendix A) is the ratified escape-hatch design and wins; §24.2's host→server
capability round-trip is NOT built and is withdrawn.
**Decision Drivers:** native capabilities are *mixed* — some sync (Storage.set, in-memory
math, clipboard read), most async (Camera.capture, permissions, location, network). The VM cannot
know which at runtime, so the contract must support both through ONE opcode pair (CALL_CAP +
AWAIT), not a sync path plus a separate async path.

## Context and Problem Statement

ADR-0044 added first-class `AWAIT`/`RESUME` (0xE0/0xE1) and a `Suspended` result to all three
VMs, and `ExprKind::Await` lowers to `AWAIT` (crates/flux-ir/src/lower/bytecode.rs:749). But the
capability half is unresolved:

- `CALL_CAP` (0x90) is still value-returning and synchronous: the oracle's impl writes a value
  into a signal and returns it (crates/flux-vm-ref/src/vm.rs:464); the native `CapabilityImpl`
  signature returns `VMValue` (runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift:18). There is
  no async capability path.
- ADR-0044 §"Lowering" says an async capability "fires the async work through the capability
  bridge (`CALL_CAP` v2 async variant or a new async cap entry)" — but the v2 async variant is
  never defined.
- `Value` (crates/flux-syntax/src/value.rs) has no `Future` variant, so there is no defined
  representation of "the thing AWAIT waits on." `AWAIT`'s `future_reg` (vm.rs:266) currently names
  "a register holding a future," but the ISA does not say what a future IS.

A capability call in MLP v2 must therefore produce *something* `AWAIT` can park on, and that
something must collapse to a plain value when the method is synchronous — without a second opcode
and without branching in user code.

### Verified current state (file:line)

- `AWAIT` = 0xE0, operands `(result_reg: u8, future_reg: u8)`; oracle decode at vm.rs:263 returns
  `ControlFlow::Suspend { resume_ip, future_reg }`. Native mirrors: OpCodes.swift:88, Opcode.kt:93.
- Async lowering emits `AWAIT result_reg=0, future_reg=fut` (bytecode.rs:756–759); `fut` is
  whatever `compile_value(inner)` produced — but `inner` cannot currently be a capability call,
  because `compile_expr_stmt` rejects `Call` (bytecode.rs:413, flux-ir owner's lane).
- `CapabilityRegistry.dev` (Registry.swift:64) and `CapabilityRegistry.default()`
  (CapabilityRegistry.kt:71) register only synchronous stubs (write a value into a signal, return
  it). No async signature exists.
- Signal graph carries `CellState { Ready(Value), Pending, Error(...) }` on both hosts:
  `CellState` is defined in `runtimes/ios/.../SignalGraph.swift` and
  `runtimes/android/.../signal/SignalGraph.kt`, and the `SignalStore` trait
  (`flux-vm-ref` `vm.rs`) exposes `cellState` / `markPending` / `resolveCell` /
  `allocate_cell`. ADR-0044 §"Signal graph" has landed.
- The oracle compiles and the async/await suspend/resume cycle is green: `SuspendState.future_reg`
  (vm.rs) is constructed by the `run` resume path, and golden ISA vectors verify the cycle.
  `CapabilityImpl` (vm.rs:190) returns a result-cell `SignalId` under the v2 contract.

## Decision Outcome

**Chosen: signal-as-future.** A capability call returns a *signal id* (a result cell), never a raw
value. The signal graph holds `CellState { Ready(Value), Pending, Error(Value) }`. `AWAIT`
parks until the cell leaves `Pending`. Sync and async capabilities differ ONLY in whether the
impl writes `Ready` before returning (sync) or later (async). No new `Value` variant, no second
async opcode, no host→server round-trip.

### 1. `CapabilityImpl` signature

Change from `(_,_,_,_) throws -> VMValue` to return a **result-cell signal id**:

```swift
// iOS (Registry.swift) — proposed
typealias CapabilityImpl = (
    _ capId: UInt32,
    _ methodId: UInt16,
    _ argument: VMValue,
    _ signals: inout SignalStore
) throws -> SignalId   // the result cell; state Ready or Pending
```

```kotlin
// Android (CapabilityRegistry.kt) — proposed
public fun interface CapabilityImpl {
    public fun call(args: FluxValue, signals: SignalStore): UInt  // signal id of the result cell
}
```

- **Sync method:** allocate a fresh cell, write `Ready(value)` into it synchronously, return its id.
- **Async method:** allocate a fresh cell (state `Pending`), return its id immediately; the host
  resolves it later — writes `Ready(v)` or `Error(e)` into that cell when the native work
  completes, which triggers the executor resume.

One signature, both shapes. The VM/lowering never branches on sync-vs-async.

### 2. `CALL_CAP` (0x90) v2 semantics

- `result_reg` ← the returned result-cell signal id.
- `args_reg` → the impl's `argument`.
- All other operand bytes unchanged (Appendix E §E.1, width `CALL_CAP`).
- The v1 golden `call_cap_basic` (capId=1, methodId=1 → writes field-0 into signal 99 and returns
  it) stays GREEN: the stub allocates cell 99, writes `Ready(field0)`, returns 99. The oracle's
  `result_reg` receives 99; a subsequent read of signal 99 yields the value. Additive change.

### 3. Lowering decides sync-vs-async from the capability IDL

Each method is declared `fn` (sync) or `async fn` (async) in the capability IDL (pending
codegen task; NOT this ADR's scope). The `flux-ir` owner's CALL_CAP-from-handler lowering
(bytecode.rs:413) targets this contract:

- **sync method:** `CALL_CAP` → read `result_reg` directly (cell already `Ready`). No `AWAIT`.
  Zero extra cost; preserves the hot synchronous path.
- **async method:** `CALL_CAP` → `AWAIT result_reg` (future_reg == result_reg). The cell id is the
  future. When it leaves `Pending`, the executor resumes and the resolved value lands in `r0`.

This is why sync methods must NOT emit `AWAIT`: a `Ready` cell continues on the next interpreter
re-entry with no real park, so the only behavioral difference is the emitted opcode, decided by the
IDL flag.

### 4. `AWAIT` runtime behavior (define precisely)

Read `cell[future_reg]`:

- `Ready(v)` → continue; `v` placed in `result_reg` (r0). No suspend. One interpreter re-entry.
- `Pending` → `Suspend` (current vm.rs:263 behavior). Executor holds `SuspendState`.
- `Error(e)` → fault the handler (surface to DevTools telemetry; do NOT resume). This is the typed
  `Camera.capture() -> Result<Photo, Denied>` path: permission denial / failure is a cell `Error`,
  not a crash.

### 5. Executor resume (both hosts)

On native resolution of the async capability (cell → `Ready`/`Error`), the executor resumes the
held `SuspendState` with `resume(state, value)` (flux-vm-ref `resume`, FluxExecutor.swift:300 reads
`state.futureReg`, FluxExecutor.kt:211 sets `futureReg`). A `Pending` cell must NOT trigger a view
mutation until `Ready` (ADR-0044 §"Signal graph"). The executor already hops eval off `@MainActor`
(FluxExecutor.swift:298, FluxExecutor.kt:165), so a park never blocks the UI thread.

### 6. Out of scope (other agents)

- Capability IDL + codegen declaring `async fn` per method and generating registry entries.
- Hello-frame capability negotiation: `handle_hello` (session.rs:120) accepts the capability list
  and drops it. Validation against the compiled tree + an `Error` frame on mismatch is a separate,
  dev-server-side task.
- §24.2 forward-RPC is withdrawn; do not build a host→server capability loop. The existing
  telemetry channel (telemetry.rs:84 — `VmStep`/`SignalWrite`/`ViewMutation`/`HandlerInvocation`)
  carries NO capability payloads and is DevTools-only; nothing is "mirrored" over it.

## Considered Options

**Option A — signal-as-future (chosen).** Capability returns a result-cell signal id; `AWAIT`
parks on cell state. Pros: one `CALL_CAP` opcode, one `AWAIT` opcode, both sync/async; reuses
existing SignalGraph; `resource`/`derived` fall out (they await a cell); no new `Value` variant;
release codegen stays a direct native call + signal write. Cons: required `CellState` enum on both
hosts (now implemented — see `SignalGraph.swift` / `SignalGraph.kt`).

**Option B — `Value::Future(handle)` + handle table.** Capability returns a `Future(u32)` handle
the executor resolves via a new handle table. Pros: explicit handle lifecycle. Cons: adds a
handle table + lifecycle to all three VMs; duplicate of the signal graph's existing pending-cell
concept; more surface to audit and keep in 3-VM agreement.

**Option C — callback model (ADR-0012, v1).** `CALL_CAP` returns immediately, host fires a
`HandlerId` later. Pros: shipped in v1. Cons: rejected for v2 — user wants first-class `await`, not
callback nesting (ADR-0044 §"Considered Options").

**Option D — forward capability calls to the dev server over WS (spec §24.2).** Cons: rejected —
on-device sensors (camera) would resolve on the MacBook; adds a latency-critical round-trip on the
hot path; violates the on-device execution model of ADR-0020. Withdrawn.

## Consequences

**Positive:**
- Uniform contract: one opcode pair serves sync and async capabilities; user code is identical
  except the `async fn` declaration in the IDL.
- No VM round-trip for capabilities; executes on-device in BOTH dev and release.
- Typed failure: denied permission / error is a cell `Error`, not a crash — mockable in unit tests.
- v1 parity preserved: `call_cap_basic` golden stays green; v1 `CALL_CAP` semantics frozen for v1
  hosts (BYTECODE_VERSION gate, ADR-0044 §"Versioning").
- Release codegen stays fast: `CALL_CAP` → direct native call; cell write is a signal assignment
  the graph already performs.

**Negative:**
- `CellState { Ready, Pending, Error }` is added to `SignalGraph` on both hosts (ADR-0044 §"Signal graph" and ADR-0045 are implemented; see `SignalGraph.swift` / `SignalGraph.kt`).
- `CapabilityImpl` signature changes in 3 runtimes (oracle + Swift + Kotlin) — a coordinated edit,
  not a one-file change.
- Precondition: the oracle must compile (vm.rs:191 `future_reg`) before async/await capability
  golden vectors can be verified.

## Implementation status

Accepted and implemented on all three runtimes (MLP v2 unified capability bridge,
commits `9cdc470` / `8c697a4` / `49c5373` and follow-ups):

- **Contract.** `CALL_CAP` (0x90) returns a result-cell signal id, not a raw value. Sync
  capabilities write `Ready` into the cell before returning; async capabilities return a `Pending`
  cell and resolve it later. `AWAIT` (0xE0) parks on cell state (ADR-0044 `SuspendState.future_reg`).
- **Oracle (`flux-vm-ref`).** `CapabilityImpl` (`vm.rs:190`) returns `SignalId`; `CapabilityRegistry`
  implements the v2 signal-id contract; `SuspendState` / `resume` are green and covered by golden
  ISA vectors.
- **iOS (`runtimes/ios`).** `CapabilityImpl` (`Registry.swift:23`) returns `UInt32` (the cell id);
  `SignalGraph.swift:30` defines `CellState { ready, pending, error(message:) }`; `FluxExecutor`
  resolves the pending cell via `asyncResolver` and `resume`s the handler (`FluxExecutor.swift:300`).
- **Android (`runtimes/android`).** `CapabilityImpl` (`CapabilityRegistry.kt:22`) `call` returns the
  allocated cell id (`signals.allocateCell()`); `SignalGraph.kt:16` defines `CellState`; the executor
  resumes on cell resolution.
- **Out of scope (still open).** Capability IDL + codegen declaring `async fn` per method, and
  `handle_hello` capability-list validation against the compiled tree, remain separate tasks (ADR-0045
  §6). The bytecode/runtime contract they target is implemented.

## References
- ADR-0044-first-class-async.md — suspend frame, AWAIT/RESUME, CellState plan.
- ADR-0020 (appendix A) — capability escape hatch; on-device callback model (§24.2 superseded).
- crates/flux-vm-ref/src/vm.rs:263,464,491 — AWAIT decode, CALL_CAP stub, SuspendState.future_reg.
- crates/flux-ir/src/lower/bytecode.rs:413,749 — CALL_CAP reject (flux-ir lane), Await lowering.
- runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift:18 — sync CapabilityImpl signature.
- runtimes/android/host/.../vm/CapabilityRegistry.kt:71 — sync default registry.
- crates/flux-syntax/src/value.rs — Value enum (no Future variant; intentionally not added).
- crates/flux-ir-serde/src/telemetry.rs:84 — DevTools telemetry events (no capability payload).
- docs/spawn/batchG/AGENT-11-integration-correction.md — CALL_CAP-from-handler lowering is the
  flux-ir owner's lane; coordinate, do not duplicate.

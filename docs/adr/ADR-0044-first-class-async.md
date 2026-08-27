# ADR-0044: First-class async in the reactive VM (MLP v2)

**Status:** Proposed
**Date:** 2026-08-27
**Supersedes (v2 scope only):** ADR-0012 "Callbacks for async (not async/await) in MLP"
**Decision Drivers:** user requirement that MLP v2 make async a first-class reactive
primitive rather than the callback model ADR-0012 chose for MLP v1; client-driven
reactivity already in place (verified: `reconcileDirty` runs with no server round-trip,
`runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift:319`).

## Context and Problem Statement

MLP v1 ships async capabilities via callbacks (ADR-0012): a `CALL_CAP` returns
immediately and the host later fires a `HandlerId` when the capability resolves. That is
adequate for MLP but leaves two gaps for v2:

1. A handler/derived that `await`s a future cannot suspend the VM — the reference VM
   (`flux-vm-ref/src/vm.rs:76` `pub fn run(...) -> Result<VmOutcome, VmError>`) and both
   native VMs run a handler to completion in one call and always return a terminal
   `VmOutcome` / `VmResult`. There is no "call me later" result.
2. The reactive graph has no pending/error cell state. `resource` and async `derived`
   cannot surface `Pending` until the future resolves.

The VM is a **flat register interpreter with no call stack**: a single `[Value; 16]`
register file, a single instruction-pointer index, and an `ip_index` loop
(`flux-vm-ref/src/vm.rs:89-379`; mirrored in `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift:98`
and `runtimes/android/host/.../vm/FluxBytecodeVM.kt:40`). Because there is no stack to
capture, a suspend is just the live interpreter state (ip, regs, gas), not a CPS
transform. This makes first-class async a small, well-bounded change.

### Current state verified in source (all claims grounded, file:line)

- **No async opcodes exist.** Opcode byte map `crates/flux-syntax/src/opcode/raw.rs`
  occupies 0x00–0xD0; the 0xE0–0xFF upper band is entirely free (0xD0 = `TO_STRING`,
  added by ADR-0043). `0xC0` is `GAS_CHECK` (raw.rs:126) — NOT free.
- **No ISA/bytecode version constant.** The only version field is `PROTOCOL_VERSION: u8 = 1`
  in `crates/flux-ir-serde/src/frame.rs:34`, which is the **wire** protocol version (Hello/
  Init/Delta frames), checked at `frame.rs:210`. The VM `run` takes raw bytecode with no
  version byte. A v2 bytecode needs its own `BYTECODE_VERSION` constant.
- **VM result types are terminal.** Swift `VmOutcome` (`FluxBytecodeVM.swift:45`); Kotlin
  `VmResult` sealed `Success | Failure` (`vm/VmResult.kt:39-46`). Neither can express
  "suspended".
- **`resource` / `effect` / `derived` are AST-only today.** `ExprKind::Resource(Box<Expr>)`
  (`crates/flux-parser/src/ast/expr.rs:181`), `LifecycleKind::Effect`/`Derived`
  (`expr.rs:207-210`). The lowering pass explicitly skips them as "not UI producers …
  handled by codegen layer from the AST" (`crates/flux-ir/src/lower/mod.rs:43-46`). There is
  **zero** dev-runtime lowering for any of them — `grep Resource|Effect|Derived|Async` in
  `flux-ir/src/lower/` returns 0 hits. They are parsed, represented, then dropped by the
  dev lowering path. So v2 async is a from-scratch lowering path, not a wiring gap.
- **Executors already separate eval from apply.** iOS: `dispatch(_:)` → `Task { @MainActor in
  … dispatch(instructions:) … reconcileDirty }` (`FluxExecutor.swift:298-330`). Android:
  `dispatch` wraps the VM call in `reactiveScope.launch { … }` (`FluxExecutor.kt:169,178`)
  and `internString` is already `suspend fun` (`FluxExecutor.kt:249`). The suspend/resume
  extension fits this shape.
- **Capability bridge exists.** `CapabilityRegistry` is a `(capId, methodId) -> impl` table
  (`runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift:39`); an async capability writes the
  eventual value into a signal. `CALL_CAP` (raw.rs:105) is the synchronous v1 path and is
  preserved unchanged for v1 parity.

## Considered Options

**Option A — Suspend frame (recommended).** Add `AWAIT` (0xE0) / `RESUME` (0xE1) opcodes and
a `Suspended` result variant carrying the live interpreter state. `run` becomes `run_resumable`
returning `RunResult::Halt(VmOutcome) | RunResult::Suspended(SuspendState)`. `resume(state,
value)` re-enters the interpreter at the saved `ip` with `value` in `r0`.

- Pros: No call stack exists, so the continuation is exactly `{ip, regs[16], gas,
  captured_signal_snapshot}` — minimal, zero allocation on the hot path beyond the suspend
  record. Both native VMs mirror it 1:1 (same flat register model). Additive: v1 sync
  opcodes untouched, v1 ISA golden vectors stay green.
- Cons: A third result variant in 3 runtimes + new ISA golden vectors.

**Option B — Callback model (status quo, ADR-0012).** Keep `CALL_CAP` + `HandlerId` callback.

- Pros: Already shipped in v1.
- Cons: User explicitly rejects this for v2 — wants first-class `await`, not callback nesting.
  Rejected for v2.

**Option C — CPS / stack-machine rewrite.** Make the VM reentrant with a captured call stack.

- Cons: The VM has no call stack by design (flat register machine, Appendix E). Inventing one
  is a large, risky change for no benefit over Option A. Rejected.

## Decision Outcome

**Chosen: Option A (suspend frame), for MLP v2 only.** ADR-0012's callback model is
**unchanged for MLP v1** — v1 parity vectors and the v1 `CALL_CAP` path are frozen.

### VM ISA (v2 addendum to Appendix E)

- `AWAIT` = `0xE0`, operands `(result_reg: u8, future_reg: u8)`, width `REG_REG`. On execute,
  the interpreter snapshots `{ip = next instruction offset, regs, gas, captured}` and returns
  `RunResult::Suspended(state)`. `future_reg` names the signal/register the resume value lands
  in; the executor arranges for the future to produce it.
- `RESUME` = `0xE1` is reserved as the symmetric wire opcode for host symmetry; the reference
  VM expresses resume via the `resume(state, value)` entry point rather than a distinct opcode
  so a suspended frame is self-describing.
- New `VmErrorKind::AwaitOutsideAsyncContext` is NOT needed: AWAIT is only emitted by v2
  lowering, so a bare AWAIT in v1 bytecode never occurs. (Defer the fault variant; do not add
  speculative error kinds.)
- Gas: AWAIT costs 1 gas (width-table discipline, ADR-0022). Remaining gas is carried in
  `SuspendState` and continues decrementing on resume.

### Versioning

- Introduce `BYTECODE_VERSION: u8` in `flux-syntax` (`crates/flux-syntax/src/opcode`). v1 emits
  `0`; v2 async closures emit `1`. This is **distinct** from `PROTOCOL_VERSION` (wire, `=1`).
- The wire frame stays `PROTOCOL_VERSION = 1`: closures ship as opaque bytecode blobs inside
  Init/Delta frames, so no frame-format change is required. A "refuse v1 host" guard is
  optional and deferred.

### Lowering (flux-ir, from-scratch)

- `resource(fn { … })` lowers to: seed a `Pending` signal, fire the async work through the
  capability bridge (`CALL_CAP` v2 async variant or a new async cap entry), emit `AWAIT`
  against the resolved-signal register, then write `Ready`/error on resume. The existing
  parity golden `B36_ASYNC` (`crates/flux-parity/src/sources.rs:124`) already authors the
  `when users.is_loading / otherwise` pending UI — v2 wires it end-to-end.
- `derived` becomes async-capable: a derived whose body `await`s yields `Pending` until resume.
- `effect` wiring (separate from async) is tracked by ADR-0044's sibling work; see
  `docs/spawn` if an effects task exists.

### Executor (both hosts)

- iOS: extend `FluxExecutor.dispatch(_:)` `Task` block to handle `RunResult::Suspended` — hold
  the `SuspendState`, resume when the future completes, then `reconcileDirty` once
  (currently at `FluxExecutor.swift:319-322`). Move the awaited eval portion off `@MainActor`
  so a suspend never blocks the UI thread.
- Android: extend `FluxExecutor.dispatch` (`FluxExecutor.kt:193`) to branch on
  `VmResult.Suspended`, resume inside `reactiveScope.launch`, reconcile once.

### Signal graph (both hosts)

- Add `enum CellState { Ready(Value), Pending, Error(...) }` to `SignalGraph`
  (`runtimes/ios/FluxHost/Sources/FluxHost/SignalGraph.swift`,
  `runtimes/android/host/.../signal/SignalGraph.kt`). A `Pending` cell does not trigger a view
  mutation until `Ready`, so the `when is_loading` pending UI stays the author's responsibility
  and the graph does not thrash mid-resolution.

## Consequences

**Positive:**
- First-class `await` with no callback nesting; matches the user's stated v2 goal.
- Minimal, allocation-light suspend (flat register machine already has no stack to capture).
- Additive: v1 sync opcodes, v1 `CALL_CAP`, and all existing ISA golden vectors stay green.
- Both native VMs share the exact suspend-frame shape, preserving the 3-runtime oracle contract
  (`flux-vm-ref` is the behavioral source of truth, `vm.rs:1-9`).

**Negative:**
- Third result variant + `SuspendState` in 3 runtimes and new ISA goldens (counter_1000/
  cond_flip-style v2 vectors).
- New `BYTECODE_VERSION` constant to maintain; risk of v1 host running v2 bytecode (mitigated by
  the optional refuse guard).

**Neutral:**
- No wire-protocol change; closures remain opaque bytecode blobs.

## References

- `crates/flux-vm-ref/src/vm.rs` — reference interpreter (oracle), `run` at :76.
- `crates/flux-syntax/src/opcode/raw.rs` — opcode bytes; 0xE0 band free; 0xC0 = `GAS_CHECK`.
- `crates/flux-ir-serde/src/frame.rs:34` — `PROTOCOL_VERSION = 1` (wire, not ISA).
- `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift:84,98` — Swift `run`.
- `runtimes/android/host/src/main/kotlin/dev/flux/host/vm/FluxBytecodeVM.kt:40` — Kotlin `run`.
- `runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift:39` — `CapabilityRegistry` bridge.
- `runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift:298-330` — iOS dispatch/queue hop.
- `runtimes/android/host/src/main/kotlin/dev/flux/host/FluxExecutor.kt:165-249` — Android dispatch.
- `crates/flux-parser/src/ast/expr.rs:181,207-210` — `Resource`/`Effect`/`Derived` AST nodes.
- `crates/flux-ir/src/lower/mod.rs:43-46` — these primitives skipped by dev lowering today.
- `crates/flux-parity/src/sources.rs:124` — `B36_ASYNC` pending-UI golden.
- ADR-0012 (appendices Appendix A) — callback model, preserved for v1.
- ADR-0043 — most recent opcode addition (`TO_STRING`/0xD0), pattern to follow for 0xE0/0xE1.
- ADR-0028 — ADR numbering: this file uses `ADR-0044` (next free numeric id past ADR-0043; not
  in the canonical `### ADR-NNNN:` sequence, so CI guard `check-adr-numbering.sh` stays green).

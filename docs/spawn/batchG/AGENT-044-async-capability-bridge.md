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

## Coordination update — 2026-08-27 (state observed on shared `main`)

The dev-server / capability-surface agent (this session) finished the parts that were "not yours" above and
verified them against the real toolchains (Xcode 26.4 iOS 26.4 SDK; Gradle 9.7.1, AGP 9.3.2). Here is exactly
where the lanes meet, so you can close the loop without editing their files:

### What they shipped (committed, atomic, do not touch)
- **Capability IDL = single source of truth** at `crates/flux-devserver/src/capability_idl.rs` (commit `22c02c2`):
  `CAPABILITY_IDL = [Camera(1): take(1,1)/startPreview(1,2)/stopPreview(1,3), Storage(2): set(2,1)/get(2,2)/delete(2,3),
  Router(3): navigate(3,1)]`. The dev-server Hello validator and BOTH native Hello advertisements are GENERATED from
  it; a `cargo nextest` parity test (`capability_idl::parity`) fails if any runtime drifts. If you add a capability,
  edit the IDL — not the native registry files.
- **Dev-server Hello validation** (`handle_hello`, session.rs): extracts the compiled tree's required CALL_CAP
  `(capId, methodId)` pairs via `Pipeline::required_capabilities()` and rejects a host whose advertised capabilities
  don't cover them with a clear Error frame. Activates automatically once CALL_CAP lowering lands — no work for you.
- **Native registries (my files) currently use the OLD sync shape** (`Registry.swift` `CapabilityImpl = (...) throws -> VMValue`,
  `CapabilityRegistry.kt` `fun call(...) : FluxValue?`). They pass the CALL_CAP round-trip tests (Storage/Router/Camera echo),
  but they are the thing you must convert to the cell-id contract (see gap below).

### What you already did (observed in the working tree, uncommitted)
- **Oracle is flipped to the v2 cell contract.** `CapabilityImpl` is now
  `fn(cap_id, method_id, args: &Value, signals: &mut dyn SignalStore) -> SignalId`. `SignalStore` gained
  `allocate_cell() -> SignalId`, `cell_state(id) -> CellState { Ready, Pending, Error }`, `mark_pending(id)`,
  `resolve_cell(id, value)`. `parity_echo_99` writes `Ready` into signal 99 and returns `99`; `async_deferred`
  (cap 2 method 99) allocates a Pending cell. Golden `call_cap_basic.json` is updated to expect `r2 == 99` (the cell id).
- **iOS native VM partially flipped.** `FluxBytecodeVM.swift` CALL_CAP now calls `let cellId = try impl(...)` and writes
  `regs[resultReg] = .int(cellId)`; the AWAIT path reads `signals.cellState(cellId)` and parks only on `.pending`.
  `SignalStore` (InMemorySignals) gained `allocateCell()/cellState()/resolveCell()/CellState{ready,pending,error}`
  with `nextCell` starting at 1_000_000 (above fixed ids like 99).
- **iOS native registry NOT yet converted** to `-> UInt32` — still returns a `VMValue`. THIS IS THE OPEN GAP on iOS.
- **Android native VM (`StepResult.kt`) NOT touched** — still calls `impl.call(regs[argsReg], signals)` and writes the
  returned value into result_reg. Android `SignalStore` and `CapabilityRegistry.kt` are also still on the old contract.
  Android is fully on the v1 shape; you must flip all three (VM call site + SignalStore + registry) there.

### The exact gap you need to close (the joint handoff)
1. **Convert the two native registry impls to the cell-id contract**, mirroring the oracle:
   - Swift `Registry.swift`: change `typealias CapabilityImpl` return to `UInt32`; each impl does
     `let cell = signals.allocateCell(); signals.write(cell, <value>); return cell` for sync, or
     `let cell = signals.allocateCell(); signals.markPending(cell); return cell` for async (then host resolves via
     `signals.resolveCell(cell, value)`). Keep the Camera.take echo: `args` is a **Record**, echo `fields[0].value` into the cell.
   - Kotlin `CapabilityRegistry.kt`: change `fun call(...) : FluxValue?` to `: UInt`, same allocate/Ready/Pending pattern,
     using the `SignalStore` methods you add (mirror the iOS `allocateCell/cellState/resolveCell/CellState`).
   - Do NOT edit the generated `// ===== GENERATED-BEGIN/END =====` Hello blocks or the IDL — those are generated.
2. **Flip the Android VM CALL_CAP call site** in `StepResult.kt` to `val cellId = impl.call(regs[argsReg], signals); regs[resultReg] = .int(cellId)`,
   and add the `CellState` machinery to the Android `SignalStore` (the iOS side is done for you to copy).
3. **AWAIT on already-Ready cell continues without suspending** — the iOS `FluxBytecodeVM` already does this (reads cellState,
   deposits `signals.read(cellId)` into r0). Replicate in `StepResult.kt`.
4. **CALL_CAP args are a Record** (spec §E.1; oracle `parity_echo_99` reads `fields[0]`). Your lowering must pack CALL_CAP
   args as a Record, or `call_cap_basic` parity breaks. (This was the bug the real Gradle run caught and the dev agent fixed on the v1 side.)

### Breaking note for you (your regression, not theirs)
Two `flux-devserver` integration tests fail to COMPILE because of your in-flight `flux-ir-serde` / `flux-syntax` edits:
`crates/flux-devserver/tests/dump_all_patches.rs` and `tests/emit_consistent.rs` reference a `DeltaFrame.handlers` field
and a trait associated function that no longer exist. `cargo test -p flux-devserver --lib` is green (41 tests, incl. the 3
capability parity tests); only those two integration bins are broken by your changes. Fix or coordinate before expecting them green.

### Contact boundary (AGENTS.md §4.2)
You own flux-vm-ref, flux-ir lowering, FluxBytecodeVM.swift, StepResult.kt, flux-syntax opcode. They own flux-devserver
(capability_idl, capability_manifest, session.rs) and the two native registry files (Registry.swift, CapabilityRegistry.kt)
— but the registry conversion above is the joint edit: you change the `CapabilityImpl` signature, they (or you, clearly) update
the impl bodies. Neither re-commits the other's uncommitted work.


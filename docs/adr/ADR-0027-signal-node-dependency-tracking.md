# ADR-0027: Signal→Node Dependency Tracking & Dirty-Set Reconciliation

**Status:** Accepted (the three-phase ladder is implemented: dirty-set reconcile + `@pure` skip in both hosts, the server-side `DependencyIndex` over `signal_deps`, and prop thunks for host-authoritative props — see "Implementation status" below) · **Owners:** P1 (iOS host), P2 (Android host), dev-server owner (wire/lowering) · **Supersedes:** none · **Depends on:** ADR-0002 (host-authoritative state), FLUX-014 (empty splices), §18.10 (`@pure`), ADR-0034-ir-node-id-bridge.md (node-ID bridge)

## Context

After a handler dispatch writes signal set `S`, the hosts currently re-apply the entire node table (Swift `FluxRuntime.dispatch` → `reconciler.apply(currentFrame())`; Kotlin mirrors via full-frame re-application). This is O(tree) per tap regardless of how many nodes actually depend on the written signals.

The signal graphs on both hosts already implement minimal notification (`observe`/`subscribe` + `flush`) — **nothing subscribes to them**. The reconciler brute-forces what the observer machinery was built to do.

## The Invariant (normative)

> After a dispatch writes signal set `S`, the only nodes whose rendered output may change are:
> (a) nodes whose prop/control expressions read some `s ∈ S`, and
> (b) nodes built/destroyed/reordered by keyed structural diffs triggered by (a).
>
> Everything else must be untouched: zero prop materializations, zero adapter calls, zero lifecycle firings.

Every phase below exists to make this invariant enforceable and observable. The trace format (reconcile-trace-format.md) is the proof mechanism.

## Decision: three-phase ladder

### Phase 1 — Cheap re-apply (host-only, no wire change)

No new data structures. Changes to the reconcile walk:

1. **Raw diff before materialization.** Compare node props as raw `VMValue`s (id-based string equality — see INV-1) and compute `lastPropHash` from raw values, not from resolved kits. Delete all `kitProps` calls used solely for hashing.
2. **Single materialization on change.** On the update path, materialize old-kit and new-kit exactly once, only when raw props differ. Build path: one kit; hash from raw values.
3. **Skip adapter calls when unchanged.** If raw props equal and child view list is identical (same ids, same order): skip `adapter.update` and `adapter.setChildren`. Emit `skip_unchanged` trace event.
4. **`@pure` ordering.** Dirty/content checks (Phase 2+) run *before* the pure hash. Pure remains the gate for ancestor-driven full-tree walks; it is redundant for direct dirty-set visits but not for Init re-applies. Keep both gates.

**Decision gate G-1 (must resolve before Phase 1 lands):** *Where do post-dispatch prop values come from today?* If the current post-dispatch re-apply provably re-applies stale props (a semantic no-op), delete it and let dispatch be VM-only until patches arrive. If any test or adapter behavior depends on the re-apply, keep it but cheap (steps 1–3). Verification owner: P1 agent, one afternoon, against current dev-server behavior.

**Per-dispatch cost after Phase 1:** N raw compares, ≤ 2·changed + built kit materializations, adapter calls only for changed nodes.

### Phase 2 — Signal deps on the wire (small wire change)

Nodes carry `signal_deps: [u32]` (see wire-signal-deps-and-thunks.md). Hosts maintain a dependency index:

```swift
@MainActor
final class DependencyIndex {
    private(set) var dependents:  [SignalId: Set<NodeId>]  // reverse index
    private(set) var nodeDeps:    [NodeId: Set<SignalId>]  // needed for O(deps) unregister
    private(set) var subtreeDeps: [NodeId: Set<SignalId>]  // nodeDeps ∪ ⋈ children, eager rollup
}
```

Kotlin mirror: same three maps, `LinkedHashMap`/`LinkedHashSet`, confined per the threading model below.

**Index lifecycle (normative table):**

| Operation | Index mutations |
|---|---|
| build node `n` | `nodeDeps[n]=deps`; `dependents[d] ∪= {n}` ∀d; rollup `subtreeDeps` up ancestor chain (O(depth)) |
| destroy subtree rooted at `n` | enumerate subtree via node table children; for each: remove from `nodeDeps`, `dependents`, `subtreeDeps`; rollup ancestors |
| replace `n` | unregister old subtree, register new |
| insert | register |
| reorder | **no index change** (same node set) |
| full frame | clear index; rebuild from frame |

**Prerequisite (hard):** P2's R7 (Android full-frame path must `destroy()` dropped nodes — currently leaks views and would leave stale index entries that phantom-dirty). This must land **before** the Android index. Swift already destroys on remove-patch; verify the full-frame path.

**Host behavior in Phase 2:** the index is *advisory* — it drives trace events (`dirty`, `skip_pruned`), validates patch scoping, and enables a scoped re-apply walk (prune any subtree with `subtreeDeps ∩ S = ∅`) if the G-1 gate kept the walk. It does **not** yet drive recomputation, because the host cannot compute new prop values without Phase 3. **Server-side payoff:** the same `signal_deps` let the dev server emit minimal Update patches addressed to `dependents[S]` instead of coarse frames.

**Degradation:** frames without the `signal_deps` flag (old server) → host falls back to Phase 1 behavior entirely. No partial-index state.

### Phase 3 — Prop thunks (host-authoritative props; ADR-0002 endgame)

Each node's prop expression ships as bytecode, exactly like handler closures (reuse the `ClosureRef` + shared blob + offset/length slicing that already exists on both hosts). Hosts run thunks locally from the dirty set; the per-tap server round-trip and the `currentFrame()` reconstruction are deleted.

**Thunk contract:**
- Entry: `r0` reserved (node context, currently unused). Signals readable via `READ_SIGNAL` as in handlers.
- Exit: on `HALT`, `r1` holds an `ALLOC_RECORD` result whose fields, in order, are the node's prop values. Node carries `prop_layout: [u16]` mapping record field position → prop index. Missing field ⇒ prop absent.
- Budgets: `THUNK_GAS = 10_000`, `THUNK_ALLOC_CAP = 1 MiB` (constants herein; ratify in Appendix E — spec owner).
- **Fault policy:** thunk fault ⇒ node keeps its prior props (render stale, never blank), error surfaces through the existing overlay path, trace records an `error` event. Never tears down the view.

**Dispatch algorithm (normative):**

```text
dispatch(event):
    (closure, bytecode) = handlerClosures[event.handlerId]  else fault
    outcome = VM.run(bytecode, closure, payload)
    if fault: surface; return
    for (id, v) in outcome.signals: graph.write(id, v)
    dirty = ⋃ dependents[s], s ∈ outcome.signals
    dirty = dirty ∩ built.keys                     // drop stale ids
    for node in dirty sorted by (depth asc, id asc):
        newProps = runThunk(node)                  // registers → record → Prop list
        if newProps == built[node].props:  emit skip_unchanged; continue
        emit update
        adapter.update(view, kit(old), kit(new))   // exactly 2 materializations, on diff only
        built[node].props = newProps
        if node.kind ∈ {If, ForEach, Match} and control prop changed:
            keyedDiff(node)                        // existing machinery: build/insert/
                                                   // remove/reorder children; registers/
                                                   // unregisters index entries; fires
                                                   // mount/cleanup; adapter.setChildren
```

**Explicitly out of scope:** the reconciler consumes the VM *outcome*, not `SignalGraph` observers. Do not subscribe the reconciler to the graph — that double-fires. Observers remain reserved for future host-side effects.

**Determinism (normative, required for trace parity):** dirty visit order is `(depth asc, id asc)`; `signals` and `dirty` trace arrays are sorted ascending. Both hosts must implement this identical rule.

## Cross-cutting invariants

**INV-1 — String id canonicality.** Within a frame generation, interned ids are unique per distinct text (forward table guarantees it; host reverse-intern must preserve it — this is why the reverse-index fix must *replace* the linear scan, not parallel it). Raw `.str` equality is id equality. **Exception:** dynamically-concatenated ids ≥ `0x8000_0000` (Kotlin's 31-bit biased range) are non-canonical — collisions are possible. Rule: if *either* compared id is ≥ `0x8000_0000`, fall back to resolved-text equality. Cost is confined to dynamic strings. Longer-term fix (wire owner): server-assigned ids for concat results; see wire-signal-deps-and-thunks.md.

**INV-2 — Trace sink is free in prod.** All trace/counters no-op when no sink is attached. Events materialize only in test/driver harnesses.

**Threading (Android, resolves R-graph):** confine the reactive core — signal graph, dependency index, closure table, string resolver, shadow tree mutations — to a single injected `reactiveDispatcher` (default `Dispatchers.Main`; tests inject a test dispatcher). `vmScope`/`Default` is demoted to deserialization only: `receiveFrame` deserializes off-main, then `withContext(reactiveDispatcher)` for everything stateful. OkHttp listener threads post onto the reactive dispatcher before touching state. Swift needs no change (already `@MainActor`); this makes the two hosts share one threading story.

## Acceptance criteria

1. Counter golden (reconcile-counters-and-budgets.md): 1,000-node tree, one `Text` bound to the counter signal — one dispatch produces ≤ 1 update, 0 builds, ≤ 2 prop materializations. **Independent of tree size.**
2. `noop_dispatch` (handler writes a signal nothing reads): zero update/build/detach events.
3. View-identity assertions (existing tests) stay green — no regression to recreate-on-update.
4. Identical reconcile traces across Swift/Kotlin hosts for the golden scripts (reconcile-trace-format.md).
5. Lifecycle parity: mount fires exactly once per built node, cleanup exactly once per detached node, verifiable via trace events.

## Implementation status

The three-phase ladder is accepted and implemented:

- **Phase 1 — cheap re-apply.** Both hosts reconcile only the dirty subset and skip `@pure`
  subtrees: `FluxExecutor.reconcile` (`runtimes/ios/.../FluxExecutor.swift`) and
  `reconcileDirty` (`runtimes/android/.../shadow/ShadowTree.kt`, driven from
  `FluxExecutor.kt`) walk `dependents[S]` rather than the whole tree; the `skip_unchanged` /
  `pure` trace events are emitted and covered by `TraceDriverTest` / `RuntimeFixesTest`.
- **Phase 2 — signal deps on the wire.** `flux-ir` `IRArena` carries `signal_deps` (FA-IRWIRE,
  ADR-0027 T13); the dev server derives a server-side `DependencyIndex` and emits minimal
  `Patch::Update`s scoped to `dependents[S]` (FA-DEVSERVER). The host index is advisory and
  degrades to the Phase-1 walk when `signal_deps` is absent (CHANGELOG "FA-DEVSERVER").
- **Phase 3 — prop thunks.** `flux-ir::lower::bytecode::compile_prop_thunk` emits the per-node
  thunk; `flux-devserver` folds `lowered.prop_thunks` into the shared closure blob so hosts run
  thunks locally from the dirty set. The dispatch algorithm in this ADR is the contract the
  hosts implement.

## Open questions (resolved)

- **OQ-1 (resolved):** post-dispatch prop values now come from the host-run prop thunk (Phase 3);
  the stale post-dispatch re-apply was removed in favour of VM-only dispatch + thunk materialisation.
- **OQ-2 (resolved):** the `signal_deps` / `prop_thunk` node sections are emitted by `flux-ir`
  (`IRArena` side-tables) and consumed by `flux-devserver`; see `wire-signal-deps-and-thunks.md`
  for the logical schema.
- **OQ-3 (resolved, MLP-scoped):** ForEach splices are empty for the MLP (FLUX-014), so
  `foreach_grow` is accepted as a gated trace scenario (CHANGELOG `flux-parity` goldens).
- **OQ-4 (out of ADR scope):** `THUNK_GAS` / `THUNK_ALLOC_CAP` ratification is a spec-owner task
  against Appendix E; the values in this ADR are the working constants the lowering uses.

# Wire additions: signal_deps & prop thunks

**Path:** docs/spec/wire-signal-deps-and-thunks.md · **Owner:** server/wire owner · **Logical schema only — byte layout is yours**

See ADR-0027 (ADR-0029 in draft) for the host-side design this wire change serves.

## Node additions

```text
Node {
    ...existing fields (id, kind, componentId, props, children, handlers,
                       span, mount, cleanup, isPure)...
    signal_deps:  [u32]        // Phase 2 — distinct READ_SIGNAL ids appearing in
                               // this node's prop AND control (cond/collection/
                               // key) expressions. Sorted ascending. May be empty.
    prop_thunk:   ClosureRef?  // Phase 3 — offset/len into the shared bytecode blob
    prop_layout:  [u16]        // Phase 3 — record-field position → prop index
}
```

## Frame gating & compatibility

- New sections are appended; presence gated by a new frame flag bit (OQ-2 assigns the bit).
- Hosts that don't understand the bit must ignore the sections (forward compat).
- A node may carry `signal_deps` without `prop_thunk` (Phase 2 server, Phase 3 host). The reverse is forbidden: thunks without deps are unusable for dirty-set pruning — reject at decode.
- `signal_deps` must be the *complete* read set of the thunk (it is derived from the same lowering pass — collect `READ_SIGNAL` operands while emitting the thunk; single source of truth).

## Server obligations

- **Phase 2:** lowering emits `signal_deps` per node; on dispatch report (handler id + written signal ids from the host), the server computes `dirty = ⋃ dependents[S]` and emits Update patches addressed only to dirty nodes (plus structural patches where control props changed).
- **Phase 3:** lowering additionally compiles each node's prop expression to a thunk and emits `prop_thunk` + `prop_layout`.
- **Dynamic string ids (INV-1 long-term fix):** server assigns canonical ids for concat results when it round-trips, or the concat opcode gains a server-visible log so the table stays canonical per generation. Until then, hosts apply the biased-id fallback rule.

## Host obligations

- Decode both sections; build the index (Phase 2) and run thunks (Phase 3) per ADR-0027.
- Dispatch report to server (Phase 2): `{handlerId, written: [u32]}` — host already knows this from `VmOutcome.signals`; no graph instrumentation needed.

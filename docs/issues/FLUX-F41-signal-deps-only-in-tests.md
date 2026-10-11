# FLUX-F41 — `set_signal_deps` is only called from tests; the minimal-patch mechanism is a production no-op

**Status:** OPEN (documented; not fixed this round)
**Found:** round-88 bug hunt (`flux-devserver` sweep)
**Severity:** medium (a documented ADR-0027 Phase 2 feature — "ship only
`dependents[written]` on a dispatch, not the whole tree" — is entirely
inactive in production; every dispatch report reaches a `None` return
and a silent no-op)

## Summary

`crates/flux-devserver/src/dispatch.rs` (the DependencyIndex) and
`Pipeline::handle_dispatch_report` are the server half of ADR-0027
Phase 2 ("signal-deps-aware minimal-patch emission"). The mechanism is
gated on `Pipeline::set_signal_deps` being called with a real
`Vec<NodeSignalDeps>` populated from the lowered IR.

A workspace-wide `grep` for `set_signal_deps`:

* `crates/flux-devserver/tests/minimal_patch.rs:145, 220` — **tests only**
* `crates/flux-devserver/src/server.rs:341` — public wrapper
  `RunningServer::set_signal_deps`
* `crates/flux-devserver/src/pipeline.rs:456` — the pipeline method

**No production caller of `RunningServer::set_signal_deps` exists.**
Nothing in `flux-devserver/src/` (server startup, watch loop, compile
pipeline, session driver) ever populates signal deps. The dependency
comment in `dispatch.rs` says the source is `Node.signal_deps` in the
lowered IR (FA-IRWIRE, T13) — but that field is not actually read from
the lowered IR by the pipeline, and `lowering → set_signal_deps` is not
wired.

## Consequence

Every host `DispatchReport` (host → server frame `0x06`) is routed by
`session.rs::handle_dispatch_report` → `Pipeline::handle_dispatch_report`
→ `emit_minimal_updates` → `Err(IndexInactive)` → `None` → no frame is
shipped. The host is left with stale props it cannot refresh, and the
"hot loop is milliseconds, bounded by `|dependents[S]|`" claim in
`reconcile-counters-and-budgets.md` is not realized. Every dispatch
report from a real host is a silent no-op.

This is orthogonal to F39 (async resume has no driver either): the two
together mean the *entire* "host reports work to the server; the server
ships a targeted incremental frame" loop described in ADR-0027 / ADR-0044
is unimplemented end-to-end in production.

## What a fix needs

Wire the source of `signal_deps` into the pipeline:

1. `flux-ir::LoweredIr` (or the `IRArena`) must expose each node's
   `signal_deps` (the distinct `READ_SIGNAL` ids its prop/control
   expressions read). FA-IRWIRE (T13) names this; the field is not
   implemented.
2. `Pipeline::compile_tree` must collect the per-node deps across all
   compiled sources, dedup, and call `set_signal_deps(Some(deps))`
   after every successful lower.
3. `Pipeline::set_signal_deps` must in turn call
   `DependencyIndex::rebuild` so the index activates.

Until that lands, the pipeline's own comment at `set_signal_deps` (and
the doc on `DependencyIndex`) is a design intent that production does
not exercise; the degrade path (F40) is the only behavior a real host
ever sees.

## Regression test

`crates/flux-devserver/tests/dispatch_degradation.rs` (shared with F40)
documents the current `None` return.

A complementary source-scan test can pin the "no production caller"
state, mirroring F39's approach.

# FLUX-F40 — Dispatch-report degradation is silent; `UnknownNode` is dropped

**Status:** OPEN (documented; not fixed this round)
**Found:** round-88 bug hunt (`flux-devserver` sweep)
**Severity:** low-medium (observability gap: when the minimal-patch
mechanism cannot run — the index is inactive, or a dirty node vanished
from the arena — the host receives nothing, and neither side gets a
diagnostic that a dispatch was silently skipped)

## Summary

`crates/flux-devserver/src/pipeline.rs::handle_dispatch_report`
documentation claims:

> `Err(_)` → **fall back to coarse frame** (the caller's
> responsibility, per the doc on `emit_minimal_updates`).

But `crates/flux-devserver/src/server/session.rs::handle_dispatch_report`
does exactly this:

    if let Some(frame) = blocking(move || {
        shared_req.pipeline.lock().handle_dispatch_report(report)
    }).await.flatten() {
        shared.broadcast(frame);
    }

The `else` branch does nothing. So:

* `emit_minimal_updates` returning `Err(IndexInactive)` (the expected
  degradation path today — no signal deps in the tree) → the pipeline
  returns `None` → the session does nothing. Correct behavior per the
  "degrade to coarse frame" design, but no diagnostic is emitted, so a
  developer who sees "dispatch report is having no effect" has no way to
  learn *why*.
* `emit_minimal_updates` returning `Err(UnknownNode(id))` (a bug —
  a dirty node id derived from the same arena vanished) → same silent
  drop. This is the path the doc explicitly says must "not ship a
  dangling patch"; the current code satisfies that (it does not ship),
  but the *diagnostic* obligation ("the caller can degrade") is unmet.

No log line is emitted on either path.

## Reproduction

Put a `tracing::info` guard on the pipeline's `handle_dispatch_report`
and drive a report from a host in a project with `signal_deps` set (see
F41 for the "never set in production" half of this). The method returns
`Some(Err(UnknownNode(…)))` — the emitted frame is an `Error` frame,
because the pipeline's `Err(_)` handler happens *after* the
`try_build_dispatch_delta` branch. Wait — re-reading the code:

    match emit_minimal_updates(written, arena, &self.index) {
        Ok(patches) if patches.is_empty() => None,
        Ok(patches) => { … },
        Err(_) => None,
    }

The `Err(_)` arm really does return `None`, not an `Error` frame. So
`UnknownNode` never reaches the caller even at the pipeline boundary —
it is silently swallowed inside `handle_dispatch_report` itself.

## What a fix needs

1. Distinguish the two error cases in `Pipeline::handle_dispatch_report`:
   * `MinimalPatchError::IndexInactive` → return `None` (correct
     degradation), but `tracing::debug!` the reason.
   * `MinimalPatchError::UnknownNode(id)` → this is a bug (a node id
     derived from the same arena vanished); return an `Error` frame so
     the host sees it, matching the H14 policy already applied to the
     `WireError` path a few lines above.

2. Emit a log line on the `Ok(patches) if patches.is_empty()` path as
   well, so a developer can distinguish "no dependents → no-op" from
   "index inactive → degraded".

## Regression test

`crates/flux-devserver/tests/dispatch_degradation.rs` — added in this
round. It drives a dispatch report through the pipeline in a state
where `signal_deps` is set to an active index that names a node id not
in the arena, and asserts the current behavior (returns `None`, no
frame) as a documented fact. When the fix lands, un-ignore and assert
an `Error` frame is returned.

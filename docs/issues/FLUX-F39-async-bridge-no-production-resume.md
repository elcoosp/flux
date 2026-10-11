# FLUX-F39 — `AsyncBridge::resume` has no production caller; parked handlers deadlock

**Status:** OPEN (documented; not fixed this round)
**Found:** round-88 bug hunt (`flux-devserver` sweep)
**Severity:** high (the async capability bridge is a documented first-class
contract per ADR-0044 / ADR-0045, but nothing in the production path can
ever complete a parked handler)

## Summary

`crates/flux-devserver/src/async_bridge.rs` documents the intended flow:

    host  --AwaitSuspend{handler, cell, resume_ip}-->  server   (park)
    host  <--Resume{handler, cell, value}------------  server   (settle)

The **park** half is wired end-to-end:

* `session.rs::handle_host_frame` matches `FRAME_AWAIT_SUSPEND` and calls
  `handle_await_suspend`, which calls `AsyncBridge::on_await_suspend` →
  `AsyncBridge::park`.

The **resume** half is **not wired at all**:

* `AsyncBridge::resume` (documented "Production resume path") has no caller
  in `crates/flux-devserver/src/`.
* `AsyncBridge::settle` and `settle_error` are `#[cfg(test)]`-only.
* No `AsyncResolver` type exists anywhere in the workspace — the doc comment
  on `resume` refers to one that was never written.
* The pipeline has no mechanism to detect that a capability has completed
  (there is no HTTP-fetch driver, no `CALL_CAP` completion hook, no async
  capability trait in the devserver).

The consequence: a host handler that hits `AWAIT` on a `Pending` result cell
**parks forever**. Its `resume_ip` sits in `AsyncBridge::parked` until the
WebSocket session closes (`serve_client` calls `clear_session` on session
end). Any synchronous user-visible effect of the awaited value never happens.

## Reproduction

At the wire level, this is trivially demonstrable:

1. Host opens a WebSocket to the dev server.
2. Host sends `Hello`, receives `Init`.
3. Host's `.flux` app has a handler containing `await SomeAsync.cap()`.
4. The handler runs, calls `CALL_CAP` for a capability that leaves the cell
   `Pending`, then `AWAIT` parks the handler.
5. Host sends `AwaitSuspend{handler, cell, resume_ip}`.
6. `handle_await_suspend` calls `bridge.park(...)`. Since there is no early
   value, `park` returns `Ok(None)`. The server sends nothing back.
7. The host's handler is now parked. Nothing in the process will ever call
   `bridge.resume(cell, value)`, because the dev server never learns that a
   capability completed (there is no such notification path).

Every test in `async_bridge.rs::tests` and `tests/bridge_lifecycle.rs`
exercises `park` and `resume` directly on the bridge — **not** through any
production capability-completion pathway, because none exists.

## What a fix needs

The bridge itself is correct; the missing half is the *capability completion
driver*. Concretely:

1. **A capability dispatch hook.** When the VM executes `CALL_CAP` and the
   registered implementation is async (leaves the cell `Pending`), something
   must eventually call `bridge.resume(cell, value)`. The `flux-types`
   comment at `capabilities/permission.rs:154` names this an "ADR-0045
   AsyncResolver" — it has not been built.

2. **A dev-server-side async capability runtime.** For an HTTP capability
   (FLUX-047), this is a `tokio` task that issues the fetch and calls
   `bridge.resume` on completion. For a host-side capability
   (`Camera.takePicture`), the host itself must report completion — but the
   wire protocol has no `CapabilityComplete` frame, only the host-side
   `Resume` direction (which is server → host, not the reverse).

3. **A wire frame for host-side capability completion.** If the value is
   produced on the host, the host must send a frame that the dev server can
   translate into a `bridge.resume` call. Appendix D §D.12 does not currently
   define such a frame; the design must add one (or decide that async
   capabilities only ever complete server-side, and build the HTTP driver).

Whichever direction is chosen, the current state is a silent deadlock, not a
sane default. The `AsyncBridge` scaffold is complete; the driver is absent.

## Regression test

`crates/flux-devserver/tests/async_bridge_reachability.rs` — added in this
round. It asserts (as a documented fact) that no non-test code in
`crates/flux-devserver/src/` calls `AsyncBridge::resume`. When the driver
lands, un-ignore and assert the frame flow through the new pathway.

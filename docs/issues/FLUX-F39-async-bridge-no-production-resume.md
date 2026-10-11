# FLUX-F39 — The dev-server `AsyncBridge` has no production caller (unused alternative path)

**Status:** OPEN (documented; scoped-out)
**Found:** round-88 bug hunt (`flux-devserver` sweep)
**Severity:** low (the dev-server side of the async capability bridge is
dead code, but the *hosts* implement their own resolver locally and
never wire to this bridge — no user-visible impact today)

## Summary (corrected)

The dev-server `AsyncBridge` is a parallel, transport-free implementation
of the ADR-0044 suspend/resume protocol:

* The **park** half is wired: `session.rs::handle_host_frame` matches
  `FRAME_AWAIT_SUSPEND` and calls `handle_await_suspend` →
  `AsyncBridge::on_await_suspend` → `AsyncBridge::park`.
* The **resume** half is not wired: `AsyncBridge::resume` has no caller
  in `crates/flux-devserver/src/`. `settle` and `settle_error` are
  `#[cfg(test)]`-only. No `AsyncResolver` type exists in the workspace.

**Correction from the first draft of this issue.** The first draft
claimed "a parked handler deadlocks in production." That is wrong:
the *hosts* implement their own resolvers locally
(`runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift`'s
`AsyncResolver` protocol, with `PassthroughAsyncResolver` /
`DelayAsyncResolver` / `CapabilityAsyncResolver`) and never send
`AwaitSuspend` over the wire to the dev server. The suspend/resume loop
closes entirely on the host, with no server participation.

No host in `runtimes/ios/` or `runtimes/android/` sends
`FRAME_AWAIT_SUSPEND`. The dev-server side is a scaffold for a
*server-driven* async capability model that was never adopted.

## Why this is filed anyway

It is dead code with a misleading *doc comment*: the `resume` method
reads "Production resume path: a capability completed externally", and
`session.rs::handle_await_suspend` handles the frame as if it were a
real production flow. A future contributor reading the code would
reasonably assume the server-side bridge is load-bearing and be
surprised to find it cannot complete a single handler.

Two clean resolutions exist:

1. **Delete it.** Remove `AsyncBridge`, `AwaitSuspendFrame`'s
   server-side handler, and the `FRAME_AWAIT_SUSPEND` dispatch arm
   from the server. The frame type stays in `flux-ir-serde` (hosts
   may still want to send it in the future), but the server no longer
   pretends to handle it.

2. **Finish it.** Build the server-side async capability runtime
   (HTTP fetch driver, host completion frame, `bridge.resume` call
   sites), so a host that *does* choose to delegate capability work
   to the server can do so. This is the ADR-0045 "server owns the
   resumption" story.

## Regression test

`crates/flux-devserver/tests/async_bridge_reachability.rs` (added in
this round) pins the "no production caller" fact via a source scan.
The test should be removed along with the code (option 1) or replaced
with an end-to-end test (option 2).

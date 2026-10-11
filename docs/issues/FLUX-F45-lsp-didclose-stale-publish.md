# FLUX-F45 — `did_change`'s debounced task still publishes after `did_close`

**Status:** OPEN (fix landed in same commit)
**Found:** round-89 bug hunt (`flux-lsp` sweep)
**Severity:** medium (a client that types a character then immediately
closes the file receives `window/publishDiagnostics` for the closed
URI *after* the empty-diagnostics publish that `did_close` sent — an
LSP-spec violation and a UX artifact: the editor may re-open or
re-flash the closed tab's squiggles)

## Summary

`crates/flux-lsp/src/lib.rs`:

* `did_change` stores a per-URI `Arc<AtomicU32>` version counter in
  `self.versions`, bumps it, clones the Arc, and spawns a debounced
  task. The task sleeps 50 ms, then checks
  `counter.load(SeqCst) != my_version` and skips publishing if a
  *newer* `did_change` has bumped the counter.
* `did_close` removes the entry from `self.versions` (dropping the
  map's Arc clone) and synchronously publishes empty diagnostics.

The bug: **the spawned task owns its own clone of the Arc, so
removing the map entry does not invalidate it.** The counter it holds
still reads `my_version`, the check passes, and 50 ms later the task
publishes stale non-empty diagnostics for a closed URI.

The close-publish (empty) and the debounced-publish (stale) can reach
the client in either order; in practice the empty one arrives first and
the stale one is a spec violation regardless.

## Reproduction

1. Open a document with a parse error.
2. Send `didChange` with a range that changes the source (bumps the
   counter to 1, spawns the debounced task).
3. Immediately send `didClose`.
4. Wait 60 ms.
5. Observe: the client receives an empty `publishDiagnostics` (from
   `did_close`), then a stale non-empty `publishDiagnostics` (from the
   debounced task).

## Fix

In `did_close`, bump the counter **before** removing the map entry so
any in-flight task observes the mismatch on wake-up:

    if let Ok(mut versions) = self.versions.lock() {
        if let Some(counter) = versions.remove(&uri) {
            // Invalidate any in-flight debounced task for this URI: it
            // holds its own Arc clone, so removing the map entry alone
            // does not stop it. Bumping the counter makes the task's
            // `counter.load() != my_version` check fail, so it skips
            // publishing — no stale diagnostics for a closed document.
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }

## Regression test

`crates/flux-lsp/tests/did_close_invalidates_pending.rs` — added in
this commit. Uses a synthetic `ClientSocket` mock to capture every
`publishDiagnostics` notification, then drives the exact sequence and
asserts no non-empty publish arrives after the close.

If a mock client is more scaffolding than is reasonable, a unit-test
approximation bumps the counter path directly: post-close, the counter
must be strictly greater than the value the pending task captured.

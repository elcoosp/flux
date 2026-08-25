---
title: Host-Authoritative State
description: Why Flux state lives in the host signal graph, not in the server, and what that implies for patching (ADR-0002).
---

Flux state is **host-authoritative**. The server never owns the runtime signal
values; it ships structure (the IR) and deltas (patches). The host owns the
signal graph, evaluates handlers, and reconciles the view.

## The invariant

> After a dispatch writes signal set `S`, the only nodes whose rendered output may
> change are (a) nodes whose prop/control expressions read some `s ∈ S`, and
> (b) nodes built/destroyed/reordered by keyed structural diffs triggered by (a).
> Everything else must be untouched.

This is normative (ADR-0027). It is what makes a 1,000-node tree cost **one
update** when you tap a counter bound to a single signal — independent of tree
size. The playground on the homepage replays exactly this scenario.

## Consequences

- **Patches are minimal.** A `count = count + 1` tap produces an `Update` patch
  addressed to the dirty node(s) only, not a re-send of the whole tree.
- **The server round-trip is deletable.** In Phase 3 (ADR-0027) prop thunks ship
  to the host, so the per-tap server round-trip and the `currentFrame()`
  reconstruction are removed entirely — the host recomputes from the dirty set.
- **Trace parity is observable.** Because state is host-owned and deterministic,
  the same frame + dispatch script run against Swift and Kotlin yields
  byte-identical traces (`reconcile-trace-format.md`).

## What this rules out

- The reconciler must **not** subscribe to the signal graph as an observer — that
  double-fires. It consumes the VM *outcome* (ADR-0027, explicitly out of scope).
- A handler that writes a signal nothing reads produces `dirty: []` and zero
  update/build/detach events (`noop_dispatch` golden).

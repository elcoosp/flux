---
title: Counter Example
description: Walk through the canonical Flux counter — state, a bound Text, and a Button tap — and how it reconciles on a tap.
---

This is the smallest interesting Flux program, and the shape the playground's
`counter_1000` golden scenario is built on.

```flux
component Counter {
  state count: Int = 0

  Column(gap: 12) {
    Text("Count: {count}")
    Button(text: "Increment", onClick: {
      count = count + 1
    })
  }
}
```

## What each line means

- `state count: Int = 0` — a mutable signal cell. The host owns `count`; the
  server only ever sees its type and initial value.
- `Text("Count: {count}")` — interpolates the signal into a string. This node's
  `signal_deps` is exactly `[1]` (the id of `count`).
- `Button(text: "Increment", onClick: { ... })` — registers handler id 7
  (in the playground's fixture) whose closure writes `count`.

## What happens on a tap

1. The `onClick` closure runs in the host VM, executing `WRITE_SIGNAL count, +1`.
2. The VM reports `signals: [1]` (the written signal id, ascending).
3. The host intersects `{1}` with each node's `signal_deps`: only the `Text`
   reads it, so `dirty: [57]` (the Text's node id), in `(depth asc, id asc)`
   order.
4. The Text re-materializes its props and fires `update` — **one** update,
   **zero** builds, ≤ 2 prop materializations. The `Column` and `Button` are
   untouched (`skip_unchanged` in a full re-apply).

Step through this exact trace on the homepage playground.

## The budgets

From `reconcile-counters-and-budgets.md`: a `counter_1000` dispatch must produce
≤ 1 update, 0 builds, ≤ 2 prop materializations — **independent of tree size**.
The general rule: every counter after a dispatch is bounded by
`|dependents[S]|` + structural-diff size, never by tree size.

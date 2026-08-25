# Reconcile counters & budgets

**Path:** docs/spec/reconcile-counters-and-budgets.md

See ADR-0027 for the design; see reconcile-trace-format.md for the trace events these counters accompany.

Both hosts expose (test-visible, cheap when unread):

```swift
struct ReconcileStats {
    var built, updated, skippedUnchanged, skippedPure, detached: Int
    var dispatches, dirtyNodesTotal, signalsWrittenTotal: Int
    var propMaterializations: Int
}
```

**Structural budgets (hard, CI-gating, per reconcile-trace-format.md scenarios):**

| Scenario | built | updated | prop_materializations | detach |
|---|---|---|---|---|
| `counter_1000` dispatch | 0 | ≤ 1 | ≤ 2 | 0 |
| `noop_dispatch` | 0 | 0 | 0 | 0 |
| `pure_subtree` (inside subtree) | 0 | 0 | 0 | 0 |
| `cond_flip` | = branch child count | — | — | = other branch count |
| `unrelated_signal` | 0 | 0 | 0 | 0 |

General rule, stated once for the agents: **all counters after a dispatch are bounded by `|dependents[S]|` + structural diff size — never by tree size.** Any test asserting a counter must express it in those terms, not absolute numbers.

**Wall-clock smoke (soft, informational, not gating):** 1,000 dispatches of `counter_1000` complete in < 100 ms on a CI-class host in a release/test build. Report in CI output; gate only if variance is tamed (median of 3 runs).

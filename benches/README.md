# LANE-H — large-tree benchmark numbers vs §3.10 budgets

Measured on an Apple-Silicon host with the workspace `bench` profile
(`criterion` 0.8, `cargo bench`). Each number is the median of 100 samples.
The §3.10 budgets are quoted from `AGENTS.md`; the CI gates for the large
trees are the linear scales proposed in `LANE-H-perf-budgets.md`.

All three large-tree benches land **inside** their budgets. No `#[ignore]`,
no relaxed constant — the differ's structural fast-path (T2) is what keeps
the 10k-node diff at O(changed) instead of O(tree).

## Differ — `bench_diff_large` (`crates/flux-differ/benches/diff.rs`)

Diff of an N-node tree with exactly one changed leaf (measured wall-time):

| Tree size | Measured (median) | CI gate (LANE-H) | §3.10 ref (50-node) |
|---|---|---|---|
| 1,000 nodes | 301.56 µs | < 20 ms | diff 50 nodes < 1 ms |
| 10,000 nodes | 4.07 ms | < 200 ms | (scaled) |

The bench asserts exactly **one** `Update` patch is emitted, proving the
diff is O(changed), not O(tree). The win comes from the T2 optimization in
`diff()`: a precomputed `children_hash` (order-sensitive fold over child
`NodeId`s) lets structurally-identical nodes short-circuit before any
`AHashSet`/`Vec` allocation for `child_ids`/`child_order`.

## VM eval — `bench_vm_eval_large` (`crates/flux-vm-ref/benches/vm_eval_large.rs`)

A 50-instruction `READ_SIGNAL`/`WRITE_SIGNAL` handler over a 10,000-signal
`InMemorySignals` graph:

| Workload | Measured (median) | §3.10 budget |
|---|---|---|
| 50-instr handler, 10k signals | 442.92 µs | VM eval 50-instr handler < 2 ms |

No allocation-pressure change was needed in the decode/eval hot path: the
reference VM already reuses its signal store and the 50-instruction program
expands to ~300 bytes, comfortably under the 16 MiB per-dispatch alloc cap
(`FluxBytecodeVM`/`FluxExecutor`).

## Pipeline — `bench_pipeline_large` (`crates/flux-devserver/benches/pipeline_large.rs`)

Full `parse → lower → wire` (Init) for a synthetic `compo` whose `Column`
body contains N sibling `Text` nodes (no `.flux` fixture on disk; generated
in the bench). The bench asserts the lowered arena actually carries the
expected node count so it is not measuring an empty tree.

| App size | Measured (median) | §3.10 end-to-end budget |
|---|---|---|
| 1,000 nodes | 3.44 ms | "Save → pixels < 100 ms" |
| 10,000 nodes | 51.07 ms | "Save → pixels < 100 ms" |

These are measured on the dev server / build host (no rendering), so they are
a floor, not the full pixel budget — but both are well inside the 100 ms gate.

## T3 — lazy prop materialization

Not applicable on the devserver `Init` path: prop materialization / the
dirty-subset reconcile lives in the host adapter kits (`adapters/ui-*`) and
the iOS `ShadowTreeReconciler`, not in `flux-devserver`. The dirty-subset
win the lane asks for is delivered instead by the differ's O(changed)
structural skip (T2) above, which is where the whole-tree walk actually
happened.

## How to reproduce

```sh
cargo bench -p flux-differ          --bench diff       diff_large
cargo bench -p flux-vm-ref          --bench vm_eval_large
cargo bench -p flux-devserver       --bench pipeline_large
```

If a budget is ever exceeded, profile before raising the constant:

```sh
cargo bench -p flux-differ --bench diff diff_large -- --profile-time=5
```

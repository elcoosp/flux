# LANE-H — Scale & performance budgets (large-tree benchmarks)

**Dispatch:** Wave 1 (independent). Do NOT delegate; Louis runs this in his own session.
**Owned directories (exclusive):**
- `crates/flux-vm-ref/src/**` (if adding eval benches)
- `crates/flux-differ/src/**` (if adding diff benches)
- `crates/flux-devserver/src/**` (if adding pipeline benches)
- `crates/flux-parity/benches/**` (existing `end_to_end.rs` — extend, don't rewrite)
**Consumed (read-only):** `flux-ir::LoweredIr`, `flux-differ::diff`, `flux-devserver::Pipeline`.
No cross-crate manifest edits (R2). No VM dispatch edits.

## Context (grounded)
Benchmarks exist for small trees and meet §3.10 budgets (parse 500 lines < 5 ms,
diff 50 nodes < 1 ms, etc.). But production apps have thousands of nodes and complex
handlers. The dirty-subset reconcile (R1) is designed O(dirty), not O(tree); the differ
walks the whole tree per update; the wire decoder allocates temporary vectors. None of
this is measured at 1k/10k-node scale. The §3.10 "Save → pixels < 100 ms" end-to-end
budget has no benchmark.

## Tasks (TDD — bench first, then optimize)
1. **Add criterion benches** for 1k and 10k-node trees:
   - `bench_diff_large` (differ): asserts diff of an N-node tree with one changed leaf is
     O(changed), measured wall-time < budget (scale the §3.10 50-node < 1 ms linearly:
     propose 1k nodes < 20 ms, 10k < 200 ms as the CI gate; record actuals).
   - `bench_vm_eval_large` (vm-ref): 50-instruction handler over a 10k-signal graph.
   - `bench_pipeline_large` (devserver): parse→lower→wire for a synthetic 10k-node app.
2. **Allocation pressure:** instrument / measure allocations in the decoder and reconciler;
   if a hot path boxes `Value` per node, introduce `SmallVec`/pre-sized `Vec` reuse or an
   arena scratch buffer. Keep changes inside the owned crate.
3. **Lazy prop materialization (optional):** if `materializeProps` still eagerly materializes
   every node on a full `Init`, gate non-dirty nodes to materialize only on first dirty
   reconcile (the iOS `ShadowTreeReconciler` already does per-node; verify the devserver
   path matches).
4. Commit a `benches/README` with measured numbers vs §3.10 budgets.

## Acceptance gates (DoD)
- `cargo bench` compiles and runs; measured numbers documented.
- If a budget is exceeded, profile (`--profile-time=5`) and land a real optimization, not a
  relaxed constant. No `#[ignore]` to hide a slow bench.
- `cargo fmt --check` / `cargo clippy -D warnings` / `cargo nextest` / `cargo doc` clean.
- `git commit --only <your bench files>` — no `git add -A`.

## Pitfalls
- Do NOT modify `flux-ir`/`flux-vm-ref` *dispatch* opcodes — only add benches or
  allocation-local optimizations in the owned crate.
- The 16 MiB per-dispatch alloc cap (`FluxBytecodeVM`/`FluxExecutor`) is a hard limit;
  optimization must stay under it, not raise it.

---
id: FLUX-079
status: todo
lane: LANE-DIFFER
phase: "Phase 0"
blocked_by: []
labels:
  - rust
  - perf
  - differ
source: FLUX_PRODUCTION_READINESS_PLAN.md §1.1 (quadratic/cubic hot path in reattach pairing) + AGENTS.md §3.10 (diff 50-node tree < 1 ms budget).
related_adrs: []
---

# FLUX-079: `flux-differ` — precompute parent/index once (kill the O(n·r·i) cold path)

- **Lane:** LANE-DIFFER (Phase 0 — fix)
- **Owner:** Rust / `flux-differ`
- **Source:** plan §1.1
- **Disjoint from:** every other issue (touches only `crates/flux-differ`).

## Problem Statement

`crates/flux-differ/src/diff.rs` has an O(n·r·i) cold path: `find_parent_and_index`
(defined at `diff.rs:251`) does a full O(n) arena scan, and it is called once per
inserted node in `diff()` (line 128) **and** inside the nested `(removed, inserted)`
loop in `reattach_pairs()` (lines 168/177) — i.e. O(r · i · n). Every `todo` add,
`ForEach` re-expand, or list splice pays this. On a 10k-node tree a modest
add/remove burst turns an otherwise-O(n) diff into seconds, defeating the
`children_hash`/`props_hash` fast path and violating the crate's own stated design
goal (AGENTS.md §3.10: diff 50-node tree < 1 ms).

## Solution

Add `build_parent_index(arena) -> AHashMap<NodeId, (NodeId, u16)>` that walks the
arena once (O(n)) and records `child -> (parent, index)`. Thread `&old_index` /
`&new_index` through `diff()` and `reattach_pairs()`; replace every
`find_parent_and_index(arena, id)` call with `index.get(&id).copied()`. This drops
the insert loop to O(n) total and `reattach_pairs` to O(r · i).

Stretch (only if the Phase 0 bench shows ForEach lists regularly exceed a few
hundred items): key `reattach_pairs` candidates on `(component_id, kind, parent,
index)` in a `HashMap` to reach O(r + i).

## Implementation Decisions

- `AHashMap` from `ahash` (already an approved hot-path dep per AGENTS.md §2.1).
- The `find_parent_and_index` free fn can be deleted once all call sites use the map.
- Keep the existing `children_hash`/`props_hash` short-circuits untouched.

## Testing Decisions

- Add `crates/flux-differ/benches/diff.rs`: diff two 10k-node arenas differing by a
  500-item list splice; assert the diff completes within the §3.10 budget.
- Gate the bench in `benchmarks.yml` / `perf-harness.yml` so this regression fails CI.

## Out of Scope

- The O(r + i) stretch optimization (deferred pending bench evidence).
- The unified reattach via content-addressed ids (FLUX-074 / ADR-0027 side-tables).

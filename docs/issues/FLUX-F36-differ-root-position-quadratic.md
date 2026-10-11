# FLUX-F36 — `root_position` / `is_root_of_new` are O(n²) per inserted root

**Status:** OPEN (fix landed in same commit)
**Found:** round-87 bug hunt (differ sweep)
**Severity:** low-medium (perf only: a hot reload that adds several
top-level components to a large arena pays O(k·n²) — k inserted roots,
each `root_position` call walks the arena twice nested. On a 10k-node
tree this is ~10⁸ operations per inserted root; with several
simultaneous top-level additions this becomes seconds.)

## Summary

`crates/flux-differ/src/diff/tree.rs`:

    pub(crate) fn is_root_of_new(new: &IRArena, id: &NodeId) -> bool {
        !new.all_ids().any(|nid| {                     // O(n)
            new.get(nid).is_some_and(|n| {
                n.children().iter().any(...)           // constant per node
            })
        })
    }

    pub(crate) fn root_position(new: &IRArena, id: &NodeId) -> u16 {
        let roots: Vec<_> = new
            .all_ids()                                  // O(n)
            .filter(|nid| {
                !new.all_ids().any(|pid| { ... })       // O(n) × O(n) = O(n²)
            })
            .collect();
        roots.iter().position(...)
    }

`root_position` is called once per inserted root from
`algorithm.rs::diff`'s insert loop. `is_root_of_new` is called on the
same code path.

## Impact

`new.all_ids()` is a `Vec::iter().copied()` — cheap per element — but the
nested loop in `root_position` is quadratic. For an arena of n nodes and
k newly-inserted roots (top-level components after a full reparse), the
cost is O(k · n²). For n = 10 000 and k = 10, that is ~10⁹ operations.

The parent index is **already precomputed** by `build_parent_index`
(FLUX-079) and available as `new_index` at the call site. A node is a
root iff `new_index` has no entry for it — `O(1)` per query.

## Fix

Compute the ordered list of roots once before the insert loop:

    let new_roots: Vec<NodeId> = new
        .all_ids()
        .filter(|nid| new_index.get(nid).is_none())
        .collect();

Then the insert loop reads the root index directly:

    } else if let Some(idx) = new_roots.iter().position(|r| r == id) {
        let n = new.get(*id).expect("present in new");
        patches.push(Patch::Insert {
            parent: synthetic_root_id(),
            index: idx as u16,
            node: to_ref(&n),
        });
    }

`is_root_of_new` and `root_position` are removed (they were only used
at this call site).

The insert loop becomes O(k · r) where r = root count, instead of
O(k · n²).

## Regression test

`crates/flux-differ/tests/insert_root_linear.rs` — a diff with 1 000
top-level components added on a 5 000-node tree completes within a
generous time bound; the previous O(n²) code makes this take seconds
and the fix makes it sub-second.

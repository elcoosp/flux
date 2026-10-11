# FLUX-F56 — `hash_fields` XOR-folds per-field digests, so duplicate props cancel

**Status:** OPEN (documented; not fixed — low severity, fix shape is a
design call)
**Found:** round-100 bug hunt (`flux-syntax/node.rs::hash_fields`)
**Severity:** low (a rare but real case where the differ's O(1) props
fast-path reports "no props change" for two nodes whose props are
actually different — one has duplicate keys that XOR-cancel to the
empty hash)

## Summary

`crates/flux-syntax/src/node.rs::hash_fields` computes an
order-independent hash of a props field set by XOR-folding each
field's per-field blake3 digest:

    fn hash_fields(fields: &[(PropIdx, Value)]) -> u64 {
        let mut accumulator: u64 = 0xcbf2_9ce4_8422_2325;
        for (index, value) in fields {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&index.to_le_bytes());
            value.hash_into(&mut hasher);
            let mut digest = [0_u8; 8];
            digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
            accumulator ^= u64::from_le_bytes(digest);
        }
        accumulator
    }

XOR has a well-known algebraic property: **two identical terms
cancel.** So any props set containing a duplicate `(PropIdx, Value)`
pair has the same hash as the same set with those duplicates removed.
In particular:

    Props::from_fields(&[(PropIdx(5), Value::Int(7)),
                         (PropIdx(5), Value::Int(7))]).hash()
    ==
    Props::from_fields(&[]).hash()
    ==
    0xcbf29ce484222325

Probe confirmed (see `crates/flux-syntax/tests/probe_r100_hash.rs` in
the round-100 log; deleted after the probe run).

## Why duplicates are reachable

The parser accepts duplicate named arguments:

* `compo C\n  let p = { a: 1, a: 2 }` — parses.
* `Card(title: "a", title: "b")` — parses.

Both go through `intern_prop_index(name)` for the key, so both entries
land in the record/props vec with the *same* `PropIdx`. Nothing
dedupes them at the boundary.

The arena's `props_hash` is computed in `Props::from_fields`, so a
node with duplicate props hashes as if it had none. The differ's
fast-path (`if props_equal && o.children_hash() == n.children_hash()`)
short-circuits on the hash check, so a props change that only adds a
duplicate field is invisible to the differ.

## Impact

Two cases:

1. **Differ misses a change.** Editing a component from
   `Card(title: "a")` to `Card(title: "a", title: "b")` — which is
   probably a user error but the parser accepts it — leaves the
   differ unable to see the change via the hash fast-path.

2. **Untrusted-input amplification.** If duplicate props ever arrive
   via the wire (not currently possible — the encoder writes the
   record as-is and the decoder reads it as-is), a hostile frame
   could produce a node whose props hash collides with the empty
   props hash.

Neither is production-critical today because real Flux users do not
write duplicate props (the type checker's `find` picks the first
match, the codegen's `HashMap::insert` picks the last, and nothing
warns). The bug is latent.

## What a fix needs

The XOR-fold trades algebraic safety for order independence. Two
alternatives:

1. **Sort then hash sequentially.** Sort `fields` by `PropIdx` before
   hashing, then chain blake3 instead of XOR-folding. Preserves
   duplicates as distinct terms (blake3 sees them in order, they hash
   differently from a single field). Costs an O(k log k) sort per
   `Props::from_fields` call — acceptable since k is the number of
   props on one node, small.

2. **Dedup at construction.** `Props::from_fields` could reject
   duplicate `PropIdx` entries (panic in debug, warn in release and
   keep the last). This is the semantically correct behavior — a
   props map keyed by `PropIdx` cannot meaningfully hold two values
   for the same key — and it eliminates the collision source rather
   than working around it.

Option 2 is the deeper fix: it makes the invariants the docs already
assert (`"keyed by PropIdx"`) true at the boundary. But it changes
what happens when a user writes `Card(title: "a", title: "b")` — the
question is whether that should be a compile error (caught at the
type-checker) or a silent last-wins (caught at `Props::from_fields`).

Filing rather than fixing because the choice of (a) reject,
(b) last-wins, (c) sort-and-chain affects the type checker's
`apply_callee` (which currently finds first-wins) and both codegen
backends (which currently do last-wins via HashMap). The three
existing consumers disagree today; the fix should reconcile them.

## Regression test

`crates/flux-syntax/tests/probe_r100_hash.rs` (deleted after the
probe run) demonstrates the collision:

    let dup = Props::from_fields(vec![
        (PropIdx::from(5u16), Value::Int(7)),
        (PropIdx::from(5u16), Value::Int(7)),
    ]);
    let empty = Props::from_fields(vec![]);
    assert_eq!(dup.hash(), empty.hash()); // passes today

When the fix lands, a test asserting `dup.hash() != empty.hash()`
should replace it.

## Related

`docs/issues/FLUX-F56` — this file.
`docs/issues/FLUX-F53-nesting-prescan-counts-comment-brackets.md` —
adjacent "best-effort heuristic" holes in the parser pipeline.

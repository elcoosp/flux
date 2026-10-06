//! Round-14 probe: emitting inserts for several *new roots* must be
//! byte-deterministic. The sort key for a root that has no parent in
//! `new_index` falls back to `(synthetic_root_id, 0)`, so multiple new roots
//! all share a key — the tie is broken by the incoming `inserted` vector
//! order, which comes from `HashSet::difference` (nondeterministic across
//! processes). If the emit order varies, the wire bytes do too.

use flux_differ::diff;
use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{ComponentId, NodeId, NodeKind, Props, Span};

fn build_two_root_tree(new_a: u32, new_b: u32) -> flux_ir::IRArena {
    let mut b = ArenaBuilder::new();
    // Two independent top-level components (no parent / no synthetic wrapper
    // in the arena — the wrapper is added by the devserver, not the differ).
    b.pack(Node {
        id: NodeId::from(new_a),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    b.pack(Node {
        id: NodeId::from(new_b),
        kind: NodeKind::Component,
        component_id: ComponentId::from(2u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    b.finish()
}

#[test]
fn two_new_roots_emit_deterministic_insert_order() {
    // Both roots present in old (ids 100/101), both gone in new (ids 200/201).
    // Every old id is removed, every new id is inserted. With the sort key
    // fallback both new roots key to `(0, synthetic_root_id, 0)` — the
    // ordering then depends on `inserted`'s initial order, which is
    // nondeterministic.
    let old = build_two_root_tree(100, 101);

    let mut hashes: std::collections::BTreeSet<u64> = Default::default();
    for _ in 0..8 {
        let new = build_two_root_tree(200, 201);
        let patches = diff(&old, &new);
        // Encode a minimal canonical form: ordered list of (parent, index).
        let mut key: Vec<(u32, u16)> = Vec::new();
        for p in &patches {
            if let flux_syntax::Patch::Insert { parent, index, .. } = p {
                key.push((u32::from(*parent), *index));
            }
        }
        // Hash the ordering.
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        hashes.insert(h.finish());
    }

    assert_eq!(
        hashes.len(),
        1,
        "two-root insert emission order is nondeterministic: {} distinct orderings over 8 runs",
        hashes.len(),
    );
}

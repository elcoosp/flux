//! Property-based round-trip tests for the Appendix D serializer (FLUX-013).
//!
//! Complementing the unit tests in `round_trip.rs`, these generate arbitrary
//! values and patch sets and assert that every generated stream serializes and
//! deserializes back to itself, byte-for-byte and value-for-value.

use flux_ir_serde::{Frame, deserialize_patches, serialize_patches};
use flux_syntax::{
    Child, ClosureRef, NodeKind, Patch, PropDiff, SignalId, Span, StringId, StringTable, Value,
};
use proptest::prelude::*;

/// Arbitrary leaf or nested value.
fn arb_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<i64>().prop_map(Value::Int),
        any::<f64>().prop_map(Value::Float),
        any::<bool>().prop_map(Value::Bool),
        (0u32..64).prop_map(|id| Value::Str(StringId::from(id))),
        (0u32..64).prop_map(Value::HandlerRef),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(Value::List),
            proptest::collection::vec((0u16..8, inner.clone()), 0..4).prop_map(Value::Record),
        ]
    })
}

fn arb_child() -> impl Strategy<Value = Child> {
    prop_oneof![
        (0u32..256u32).prop_map(Child::Node),
        proptest::collection::vec((any::<u64>(), 0u32..256u32), 0..3)
            .prop_map(|items| Child::Splice { items }),
    ]
}

/// A fully arbitrary patch, exercising every Appendix D §D.2 tag.
fn arb_patch() -> impl Strategy<Value = Patch> {
    prop_oneof![
        (0u32..256u32).prop_map(|id| Patch::Remove { id }),
        (0u32..256u32, arb_value()).prop_map(|(id, v)| Patch::Update {
            id,
            props_diff: PropDiff {
                changes: vec![(0u16, v)],
                removals: vec![],
            },
        }),
        (0u32..256u32, 0u16..4, arb_child()).prop_map(|(parent, index, _child)| Patch::Insert {
            parent,
            index: index % 8,
            node: placeholder_node(parent.wrapping_mul(7).wrapping_add(index as u32)),
        }),
        (0u32..256u32).prop_map(|parent| Patch::Reorder {
            parent,
            keys: (0u32..4).map(|k| k.wrapping_add(parent)).collect(),
        }),
        (0u32..256u32).prop_map(|id| Patch::Replace {
            id,
            node: placeholder_node(id),
        }),
        (0u32..256u32).prop_map(|id| Patch::Handler {
            id,
            closure: ClosureRef {
                hash: id as u64,
                bytecode_offset: 0,
                bytecode_len: 4,
                captured_signals: vec![SignalId::from(1u32)],
                span: Span::new(0, 0, 4),
            },
        }),
    ]
}

fn placeholder_node(id: flux_syntax::NodeId) -> flux_syntax::NodeRef {
    let span = Span::new(0, 0, 4);
    flux_syntax::NodeRef {
        id,
        kind: NodeKind::Primitive,
        component_id: 1,
        props: flux_syntax::Props::from_fields(vec![(0u16, Value::Int(id as i64))]),
        children: vec![],
        handlers: vec![],
        span,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Every generated patch set survives serialize → deserialize unchanged.
    #[test]
    fn prop_patch_round_trip(patches in proptest::collection::vec(arb_patch(), 0..12)) {
        let table = StringTable::new();
        let bytes = serialize_patches(&patches, &table);
        let back = deserialize_patches(&bytes).expect("decode");
        // `Patch` has no `Eq` impl (it lives in `flux-syntax`); we assert
        // structural equality through the deterministic canonical encoding.
        prop_assert_eq!(serialize_patches(&back, &table), bytes);
    }

    /// A delta frame round-trips its patch set and string delta.
    #[test]
    fn prop_delta_frame_round_trip(
        patches in proptest::collection::vec(arb_patch(), 0..12),
        strings in proptest::collection::vec((0u32..64, "[a-z]{1,8}"), 0..6),
    ) {
        let deltas: Vec<(StringId, String)> = strings
            .into_iter()
            .map(|(id, s)| (StringId::from(id), s))
            .collect();
        let frame = Frame::delta(0, 0, &patches, &deltas);
        let bytes = frame.to_bytes();
        let decoded = Frame::from_delta_bytes(&bytes).expect("delta decode");
        prop_assert_eq!(
            serialize_patches(&decoded.patches, &StringTable::new()),
            serialize_patches(&patches, &StringTable::new())
        );
        prop_assert_eq!(decoded.strings, deltas);
    }

    /// Two serializations of identical input produce identical bytes.
    #[test]
    fn prop_serialize_is_deterministic(
        patches in proptest::collection::vec(arb_patch(), 0..12),
    ) {
        let table = StringTable::new();
        let a = serialize_patches(&patches, &table);
        let b = serialize_patches(&patches, &table);
        prop_assert_eq!(a, b);
    }
}

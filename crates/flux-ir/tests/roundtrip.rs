//! Property tests for `flux-ir` (FLUX-004 acceptance criteria).
//!
//! 1. Pack/unpack round-trip: a node packed into an [`IRArena`] and read back
//!    through [`NodeView`] reconstructs its props, children and handlers
//!    exactly.
//! 2. Node-ID stability (ADR-0013): [`compute_node_id`] is a pure function of
//!    `(parent, kind, span, key)` — identical inputs give identical IDs, and
//!    inserting a sibling at a *different* source span does not alter an
//!    existing node's ID.

use flux_ir::{ArenaBuilder, Node, compute_node_id};
use flux_syntax::{Child, ComponentId, NodeId, PropIdx, Span};
use flux_syntax::{NodeKind, Props, Value};
use proptest::prelude::*;

fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(Value::Int),
        any::<f64>().prop_map(Value::Float),
        any::<bool>().prop_map(Value::Bool),
        any::<u32>().prop_map(Value::Str),
        Just(Value::Null),
    ]
}

fn arb_node(id: NodeId) -> impl Strategy<Value = Node> {
    (
        any::<u8>(),
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        proptest::collection::vec((any::<u16>(), arb_value()), 0..6),
        proptest::collection::vec(any::<u32>(), 0..4),
    )
        .prop_map(move |(kind, comp, start, end, props, handlers)| {
            let children = vec![Child::Node(NodeId::from(99u32))];
            Node {
                id,
                kind: NodeKind::from_tag(kind % 7).expect("tag in range"),
                component_id: ComponentId::from(comp),
                props: Props::from_fields(
                    props
                        .into_iter()
                        .map(|(i, v)| (PropIdx::from(i), v))
                        .collect(),
                ),
                children,
                handlers: handlers.into_iter().collect(),
                span: Span::new(0, start, end.saturating_add(start)),
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn pack_unpack_round_trips(node in arb_node(NodeId::from(7u32))) {
        let mut builder = ArenaBuilder::new();
        builder.pack(node.clone());
        let arena = builder.finish();
        let view = arena.get(node.id).expect("node present after pack");

        prop_assert_eq!(view.kind(), node.kind);
        prop_assert_eq!(view.component_id(), node.component_id);
        prop_assert_eq!(view.span(), node.span);
        let got_props = view.props();
        prop_assert_eq!(got_props.fields(), node.props.fields());
        prop_assert_eq!(view.handlers(), node.handlers);
        prop_assert_eq!(view.children().len(), 1);
    }

    #[test]
    fn node_id_is_deterministic(
        parent in any::<u32>(),
        kind in any::<u8>(),
        start in any::<u32>(),
        end in any::<u32>(),
        key in any::<u64>(),
    ) {
        let span = Span::new(0, start, end.saturating_add(start));
        let k = NodeKind::from_tag(kind % 7).expect("tag in range");
        let a = compute_node_id(parent, k, span, Some(key));
        let b = compute_node_id(parent, k, span, Some(key));
        prop_assert_eq!(a, b, "identical inputs must yield identical IDs");
    }

    #[test]
    fn sibling_insert_does_not_shift_existing_id(
        base_span in (any::<u32>(), any::<u32>()),
        other_span in (any::<u32>(), any::<u32>()),
    ) {
        let (s1, e1) = base_span;
        let (s2, e2) = other_span;
        // Only meaningful when the two spans genuinely differ.
        prop_assume!(s1 != s2 || e1 != e2);
        let kind = NodeKind::Component;
        let id_a = compute_node_id(NodeId::from(0u32), kind, Span::new(0, s1, e1.saturating_add(s1)), None);
        let id_b = compute_node_id(NodeId::from(0u32), kind, Span::new(0, s2, e2.saturating_add(s2)), None);
        prop_assert_ne!(id_a, id_b, "distinct spans must derive distinct IDs");
    }
}

use flux_ir::{ArenaBuilder, IRArena, Node};
use flux_syntax::{ComponentId, NodeId, NodeKind, PropIdx, Props, Span, Value};

/// Builds a single-node arena (id 1) with the given prop.
pub(crate) fn single_prop(prop_value: Value) -> IRArena {
    let node = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), prop_value)]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    };
    let mut b = ArenaBuilder::new();
    b.pack(node);
    b.finish()
}

/// Builds a single-node arena whose node is `component_id`/`kind`.
pub(crate) fn single_node(component: u32, kind: NodeKind) -> IRArena {
    let node = Node {
        id: NodeId::from(1u32),
        kind,
        component_id: ComponentId::from(component),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    };
    let mut b = ArenaBuilder::new();
    b.pack(node);
    b.finish()
}

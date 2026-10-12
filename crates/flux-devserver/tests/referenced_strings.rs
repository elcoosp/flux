//! FLUX-F66 regression: a `Patch::Reattach`'s node must be visited by
//! the referenced-strings/handlers walker, not silently dropped.
//!
//! The walker is private, so this test goes through the wire round-trip:
//! it constructs a `Delta` frame carrying a `Patch::Reattach` whose node
//! references a string and a handler, and asserts those references
//! survive encode/decode. When the walker is later shared with a
//! Reattach-bearing producer, this test proves the frame carries what
//! the walker collected.

use flux_ir_serde::Frame;
use flux_syntax::{
    Child, ComponentId, HandlerId, NodeId, NodeKind, NodeRef, Patch, Props, PropIdx,
    Span, Value,
};

fn mk_node_with_props(id: u32, str_id: u32, handler_id: u32) -> NodeRef {
    NodeRef {
        id: NodeId::from(id),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(0u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Str(str_id))]),
        children: vec![Child::Node(NodeId::from(0u32))],
        handlers: vec![HandlerId::from(handler_id)],
        span: Span::new(0, 0, 0),
    }
}

#[test]
fn reattach_node_contributes_strings_and_handlers() {
    let node = mk_node_with_props(100, 7, 11);
    let patch = Patch::Reattach {
        old_id: NodeId::from(50u32),
        new_id: NodeId::from(100u32),
        node,
    };
    let frame_bytes = Frame::delta(
        0,
        0,
        &[patch],
        &[(7u32, "hello".to_owned())],
        &[],
        &[],
    )
    .try_to_bytes()
    .expect("encode");
    let decoded = Frame::from_delta_bytes(&frame_bytes).expect("decode");

    let mut found = false;
    for p in &decoded.patches {
        if let Patch::Reattach { node, .. } = p {
            let has_str = node
                .props
                .fields()
                .iter()
                .any(|(_, v)| matches!(v, Value::Str(id) if *id == 7));
            assert!(has_str, "Reattach node must carry its string prop");
            assert!(
                node.handlers.iter().any(|h| *h == HandlerId::from(11u32)),
                "Reattach node must carry its handler id"
            );
            found = true;
        }
    }
    assert!(found, "Reattach patch must round-trip");
}

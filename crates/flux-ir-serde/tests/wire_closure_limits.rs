//! F5 regression: `write_closures` must return a `WireError` on duplicate
//! handler ids and oversized bytecode, not panic or silently emit an empty ref.

use flux_ir::ClosureIR;
use flux_ir_serde::{Frame, WireError};
use flux_syntax::{
    Child, ComponentId, HandlerId, NodeId, NodeKind, NodeRef, Props, SignalId, Span, StringTable,
};

fn closure(id: u32, bytecode: Vec<u8>) -> ClosureIR {
    ClosureIR::new(
        HandlerId::from(id),
        bytecode,
        vec![SignalId::from(1u32)],
        Span::new(0, 0, 0),
    )
}

fn stub_root() -> NodeRef {
    NodeRef {
        id: NodeId::from(1u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(0u32),
        props: Props::default(),
        children: Vec::<Child>::new(),
        handlers: Vec::new(),
        span: Span::new(0, 0, 0),
    }
}

#[test]
fn duplicate_handler_ids_return_wire_error() {
    let c = closure(7, vec![0x00]);
    let table = StringTable::new();
    let frame = Frame::init(
        &stub_root(),
        &[],
        &[],
        &[],
        &table,
        &[],
        &[c.clone(), c.clone()],
        &[],
    );
    match frame.try_to_bytes() {
        Err(WireError::DuplicateHandlerId { id }) => assert_eq!(id, 7),
        other => panic!("expected DuplicateHandlerId, got {other:?}"),
    }
}

#[test]
fn oversized_handler_bytecode_returns_wire_error() {
    let huge = vec![0u8; usize::from(u16::MAX) + 1];
    let c = closure(7, huge);
    let table = StringTable::new();
    let frame = Frame::init(&stub_root(), &[], &[], &[], &table, &[], &[c], &[]);
    assert!(
        matches!(
            frame.try_to_bytes(),
            Err(WireError::LengthExceedsU16 { .. })
        ),
        "oversized handler body must produce a LengthExceedsU16 error",
    );
}

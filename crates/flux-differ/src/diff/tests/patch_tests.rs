use super::super::*;
use super::common::*;
use flux_syntax::{ComponentId, NodeId, NodeKind, Patch, PropIdx, Value};

#[test]
fn identical_hash_emits_no_update() {
    // Same prop value -> identical Props::hash -> no Update patch.
    let a = single_prop(Value::Int(12));
    let b = single_prop(Value::Int(12));
    let patches = diff(&a, &b);
    assert!(
        patches.is_empty(),
        "identical-hash nodes must emit no patch"
    );
    // The arena hash and Props::hash agree.
    let va = a.get(NodeId::from(1u32)).unwrap();
    let vb = b.get(NodeId::from(1u32)).unwrap();
    assert_eq!(va.props_hash(), vb.props_hash());
    assert_eq!(va.props_hash(), va.props().hash());
}

#[test]
fn different_hash_emits_correct_update() {
    let a = single_prop(Value::Int(12));
    let b = single_prop(Value::Int(99));
    let patches = diff(&a, &b);
    assert_eq!(
        patches.len(),
        1,
        "different-hash must emit exactly one patch"
    );
    match &patches[0] {
        Patch::Update { id, props_diff } => {
            assert_eq!(*id, NodeId::from(1u32));
            assert_eq!(
                props_diff.changes,
                vec![(PropIdx::from(0u16), Value::Int(99))]
            );
        }
        other => panic!("expected Update, got {other:?}"),
    }
}

#[test]
fn component_swap_at_same_node_reattaches_instead_of_replacing() {
    // `Column` → `Row` at the same node id: state must survive, so the
    // differ emits a state-preserving Reattach, never a Replace.
    let a = single_node(1, NodeKind::Primitive);
    let b = single_node(2, NodeKind::Primitive);
    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1);
    match &patches[0] {
        Patch::Reattach {
            old_id,
            new_id,
            node,
        } => {
            assert_eq!(*old_id, NodeId::from(1u32));
            assert_eq!(*new_id, NodeId::from(1u32));
            assert_eq!(node.component_id, ComponentId::from(2u32));
        }
        other => panic!("expected Reattach, got {other:?}"),
    }
    assert!(patches[0].is_state_preserving());
}

#[test]
fn kind_change_still_replaces() {
    // A genuine node-kind change (Primitive → If) is not the same construct
    // and must NOT silently inherit another node's state.
    let a = single_node(1, NodeKind::Primitive);
    let b = single_node(1, NodeKind::If);
    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1);
    assert!(matches!(&patches[0], Patch::Replace { .. }));
}

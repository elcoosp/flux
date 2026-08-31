use flux_syntax::{ComponentId, EffectId, InstanceId, NodeId, SignalId, StringId, Value};

use super::*;

#[test]
fn register_allocates_monotonic_ids() {
    let mut reg = InstanceRegistry::new();
    let a = reg.register(ComponentInstance::new(
        InstanceId::from(0u32),
        ComponentId::from(1u32),
        NodeId::from(10u32),
    ));
    let b = reg.register(ComponentInstance::new(
        InstanceId::from(0u32),
        ComponentId::from(1u32),
        NodeId::from(11u32),
    ));
    assert_eq!(a, InstanceId::from(1u32));
    assert_eq!(b, InstanceId::from(2u32));
    assert_ne!(a, b);
}

#[test]
fn register_preserves_explicit_id() {
    let mut reg = InstanceRegistry::new();
    let id = reg.register(ComponentInstance::new(
        InstanceId::from(42u32),
        ComponentId::from(1u32),
        NodeId::from(10u32),
    ));
    assert_eq!(id, InstanceId::from(42u32));
    assert_eq!(
        reg.next_id,
        InstanceId::from(1u32),
        "explicit id must not advance allocator"
    );
}

#[test]
fn node_lookup_resolves_instance() {
    let mut reg = InstanceRegistry::new();
    reg.register(ComponentInstance::new(
        InstanceId::from(0u32),
        ComponentId::from(1u32),
        NodeId::from(10u32),
    ));
    let found = reg
        .instance_for_node(NodeId::from(10u32))
        .expect("instance should be findable by node");
    assert_eq!(found.node_id, NodeId::from(10u32));
}

#[test]
fn unregister_clears_node_mapping() {
    let mut reg = InstanceRegistry::new();
    let id = reg.register(ComponentInstance::new(
        InstanceId::from(0u32),
        ComponentId::from(1u32),
        NodeId::from(10u32),
    ));
    assert!(reg.instance_for_node(NodeId::from(10u32)).is_some());
    reg.unregister(id);
    assert!(reg.instance_for_node(NodeId::from(10u32)).is_none());
}

/// Registers one instance carrying live state, so a reattach can be shown to
/// preserve it.
fn registry_with_stateful_instance() -> (InstanceRegistry, InstanceId) {
    let mut reg = InstanceRegistry::new();
    let mut instance = ComponentInstance::new(
        InstanceId::from(0u32),
        ComponentId::from(7u32),
        NodeId::from(10u32),
    );
    instance.signals.push(SignalId::from(99u32));
    instance.effects.push(EffectId::from(5u32));
    instance.state.push((StringId::from(3u32), Value::Int(42)));
    let id = reg.register(instance);
    (reg, id)
}

#[test]
fn reattach_moves_the_instance_to_the_new_node() {
    let (mut reg, id) = registry_with_stateful_instance();
    let moved = reg
        .try_reattach(
            NodeId::from(10u32),
            NodeId::from(20u32),
            ComponentId::from(8u32),
        )
        .expect("a live instance is reattachable");
    assert_eq!(moved, id, "the instance identity must survive the reattach");
    assert!(
        reg.instance_for_node(NodeId::from(10u32)).is_none(),
        "the old node mapping must be released"
    );
    let found = reg
        .instance_for_node(NodeId::from(20u32))
        .expect("the new node resolves to the same instance");
    assert_eq!(found.id, id);
}

#[test]
fn reattach_preserves_signals_effects_and_state() {
    let (mut reg, _) = registry_with_stateful_instance();
    reg.try_reattach(
        NodeId::from(10u32),
        NodeId::from(20u32),
        ComponentId::from(8u32),
    )
    .expect("reattaches");
    let found = reg
        .instance_for_node(NodeId::from(20u32))
        .expect("instance present");
    assert_eq!(found.signals, vec![SignalId::from(99u32)]);
    assert_eq!(found.effects, vec![EffectId::from(5u32)]);
    assert_eq!(found.state, vec![(StringId::from(3u32), Value::Int(42))]);
}

#[test]
fn reattach_adopts_the_new_component_id() {
    // `Column` -> `Row` is exactly a component_id change at a live node.
    let (mut reg, _) = registry_with_stateful_instance();
    reg.try_reattach(
        NodeId::from(10u32),
        NodeId::from(20u32),
        ComponentId::from(8u32),
    )
    .expect("reattaches");
    let found = reg
        .instance_for_node(NodeId::from(20u32))
        .expect("instance present");
    assert_eq!(found.component_id, ComponentId::from(8u32));
}

#[test]
fn reattach_in_place_keeps_the_same_node_id() {
    // The differ's component_id-changed-at-a-stable-id case: old == new.
    let (mut reg, id) = registry_with_stateful_instance();
    let moved = reg
        .try_reattach(
            NodeId::from(10u32),
            NodeId::from(10u32),
            ComponentId::from(8u32),
        )
        .expect("an in-place reattach is valid");
    assert_eq!(moved, id);
    let found = reg
        .instance_for_node(NodeId::from(10u32))
        .expect("still resolvable at the same node");
    assert_eq!(found.component_id, ComponentId::from(8u32));
}

#[test]
fn reattach_refuses_an_unknown_old_node() {
    let (mut reg, _) = registry_with_stateful_instance();
    assert!(
        reg.try_reattach(
            NodeId::from(999u32),
            NodeId::from(20u32),
            ComponentId::from(8u32)
        )
        .is_none(),
        "there is nothing to transfer, so the caller must fall back to Replace"
    );
}

#[test]
fn reattach_refuses_to_orphan_a_live_target_instance() {
    let (mut reg, _) = registry_with_stateful_instance();
    let occupant = reg.register(ComponentInstance::new(
        InstanceId::from(0u32),
        ComponentId::from(2u32),
        NodeId::from(20u32),
    ));
    assert!(
        reg.try_reattach(
            NodeId::from(10u32),
            NodeId::from(20u32),
            ComponentId::from(8u32)
        )
        .is_none(),
        "reattaching onto a live node would orphan its instance"
    );
    assert_eq!(
        reg.instance_for_node(NodeId::from(20u32))
            .map(|instance| instance.id),
        Some(occupant),
        "the occupant must be left untouched"
    );
}

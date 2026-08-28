//! Live component-instance tracking (Appendix C §C.2).
//!
//! When the dev server ships a tree to the host app, the host materialises a
//! [`ComponentInstance`] per live component node. The [`InstanceRegistry`]
//! maps both `InstanceId` and the originating [`NodeId`] to each instance, so a
//! hot-swapped tree can preserve signal state and effects across edits (ASR-003).

use ahash::AHashMap;
use flux_syntax::Value;
use flux_syntax::{ComponentId, EffectId, InstanceId, NodeId, SignalId, StringId};

/// One live component instance in the host app.
///
/// Holds the signals and effects this instance owns, the closures bound to its
/// handlers, and its child instances. `state` captures the initial state values
/// so a re-lowered tree can be reconciled against the running instance.
#[derive(Clone, Debug)]
pub struct ComponentInstance {
    /// Stable instance identity assigned by the host.
    pub id: InstanceId,
    /// Interned component/primitive name.
    pub component_id: ComponentId,
    /// The IR node this instance was materialised from.
    pub node_id: NodeId,
    /// Signals owned by this instance.
    pub signals: Vec<SignalId>,
    /// Effects owned by this instance.
    pub effects: Vec<EffectId>,
    /// Closures (handlers) bound by this instance.
    pub closures: Vec<flux_syntax::HandlerId>,
    /// Child instances, in render order.
    pub children: Vec<InstanceId>,
    /// Initial state values, keyed by interned state name.
    pub state: Vec<(StringId, Value)>,
}

impl ComponentInstance {
    /// Creates a bare instance; collections are filled in as the tree is walked.
    #[must_use]
    pub fn new(id: InstanceId, component_id: ComponentId, node_id: NodeId) -> Self {
        Self {
            id,
            component_id,
            node_id,
            signals: Vec::new(),
            effects: Vec::new(),
            closures: Vec::new(),
            children: Vec::new(),
            state: Vec::new(),
        }
    }
}

/// Registry of every live [`ComponentInstance`].
///
/// `node_to_instance` lets a diff that finds a changed [`NodeId`] locate the
/// instance whose state must be preserved. `next_id` is the allocator for new
/// instances, kept monotonic so an ID is never reused after destruction.
#[derive(Clone, Debug, Default)]
pub struct InstanceRegistry {
    /// Instances keyed by [`InstanceId`].
    pub instances: AHashMap<InstanceId, ComponentInstance>,
    /// Reverse map from originating [`NodeId`] to its instance.
    pub node_to_instance: AHashMap<NodeId, InstanceId>,
    /// Monotonic allocator for new instance IDs.
    pub next_id: InstanceId,
}

impl InstanceRegistry {
    /// Creates an empty registry with the first instance ID (`1`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            instances: AHashMap::new(),
            node_to_instance: AHashMap::new(),
            next_id: InstanceId::from(1u32),
        }
    }

    /// Registers `instance`, allocating a fresh [`InstanceId`] when
    /// `instance.id` is `0`.
    ///
    /// Returns the ID the instance ended up with.
    pub fn register(&mut self, mut instance: ComponentInstance) -> InstanceId {
        if instance.id == InstanceId::from(0u32) {
            instance.id = self.next_id;
            self.next_id = InstanceId::from(self.next_id + 1);
        }
        self.node_to_instance.insert(instance.node_id, instance.id);
        let id = instance.id;
        self.instances.insert(id, instance);
        id
    }

    /// Returns the instance for `node_id`, if one was registered.
    #[must_use]
    pub fn instance_for_node(&self, node_id: NodeId) -> Option<&ComponentInstance> {
        let id = self.node_to_instance.get(&node_id)?;
        self.instances.get(id)
    }

    /// Removes an instance, freeing its node mapping.
    ///
    /// Returns the removed instance when present.
    pub fn unregister(&mut self, id: InstanceId) -> Option<ComponentInstance> {
        let removed = self.instances.remove(&id)?;
        self.node_to_instance.remove(&removed.node_id);
        Some(removed)
    }

    /// Transfers the instance registered for `old_node_id` onto `new_node_id`,
    /// adopting `new_component_id`, instead of destroying and re-creating it.
    ///
    /// This is the host-side half of `Patch::Reattach` (Appendix D §D.2 tag
    /// `0x07`): the instance keeps its `InstanceId`, its signals, effects,
    /// closures and captured `state`, so a structural edit (`Column` → `Row`,
    /// a re-spanned node) no longer resets input focus or scroll position.
    /// `new_component_id` is expected to differ from the instance's current one —
    /// that is precisely the case a plain `Replace` used to handle destructively.
    ///
    /// Eligibility (matching component name modulo generics, stable parent and
    /// order) is decided by the differ before the patch is emitted; this method
    /// is the mechanical transfer and only refuses the two cases that would
    /// corrupt the registry:
    ///
    /// - no instance is registered for `old_node_id` — nothing to transfer;
    /// - `new_node_id` already has a live instance — reattaching would orphan it.
    ///
    /// Returns the transferred [`InstanceId`] on success, or `None` when
    /// refused; a refusal means the caller must fall back to a full `Replace`.
    pub fn try_reattach(
        &mut self,
        old_node_id: NodeId,
        new_node_id: NodeId,
        new_component_id: ComponentId,
    ) -> Option<InstanceId> {
        if old_node_id != new_node_id && self.node_to_instance.contains_key(&new_node_id) {
            return None;
        }
        let id = *self.node_to_instance.get(&old_node_id)?;
        let instance = self.instances.get_mut(&id)?;
        instance.node_id = new_node_id;
        instance.component_id = new_component_id;
        self.node_to_instance.remove(&old_node_id);
        self.node_to_instance.insert(new_node_id, id);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
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
}

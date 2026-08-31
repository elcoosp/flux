use super::component_instance::ComponentInstance;
use ahash::AHashMap;
use flux_syntax::{ComponentId, InstanceId, NodeId};

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

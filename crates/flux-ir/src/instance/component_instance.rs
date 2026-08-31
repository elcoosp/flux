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

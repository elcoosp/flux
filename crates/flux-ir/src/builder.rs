//! Hand-construction API for [`IRArena`] (Appendix C §C.1, acceptance #5).
//!
//! The differ, codegen and parity suites need to build small trees without a
//! parser. [`ArenaBuilder`] is a thin, fallible-free front end that accepts
//! fully-formed [`Node`] inputs and packs them in declaration order.

use flux_syntax::{Child, ComponentId, HandlerId, NodeId, Span, StringId};
use flux_syntax::{NodeKind, Props, StringTable};

use crate::arena::{IRArena, NodeView};
use crate::closure::ClosureIR;

/// A fully-specified node, ready to pack into an [`IRArena`].
///
/// Unlike [`NodeRef`](flux_syntax::NodeRef) (which is the parser's output), a
/// `Node` carries its own [`NodeId`] — the caller is responsible for deriving
/// it via [`compute_node_id`](crate::compute_node_id).
#[derive(Clone, Debug)]
pub struct Node {
    /// Stable, source-derived identity.
    pub id: NodeId,
    /// Node kind.
    pub kind: NodeKind,
    /// Interned component/primitive name.
    pub component_id: ComponentId,
    /// Prop map.
    pub props: Props,
    /// Child slots in render order.
    pub children: Vec<Child>,
    /// Bound handlers.
    pub handlers: Vec<HandlerId>,
    /// Source span.
    pub span: Span,
}

/// Front end for packing nodes (and their closures) into an [`IRArena`].
///
/// The builder owns a [`StringTable`] that is threaded into the finished
/// [`IRArena`] (see [`ArenaBuilder::intern_string`] and
/// [`ArenaBuilder::finish`]). This closes the "string-table gap" (Gap G3):
/// identifiers interned during construction remain resolvable from the packed
/// arena, so downstream wire codec / adapters can recover `Value::Str` text.
///
/// # Examples
///
/// ```
/// use flux_ir::{ArenaBuilder, Node, compute_node_id};
/// use flux_syntax::{NodeKind, Props, Span};
///
/// let mut builder = ArenaBuilder::new();
/// let id = compute_node_id(0, NodeKind::Component, Span::new(0, 0, 4), None);
/// builder.pack(Node {
///     id,
///     kind: NodeKind::Component,
///     component_id: 1,
///     props: Props::default(),
///     children: vec![],
///     handlers: vec![],
///     span: Span::new(0, 0, 4),
/// });
/// let arena = builder.finish();
/// assert_eq!(arena.len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct ArenaBuilder {
    arena: IRArena,
    /// Caller-supplied interner, threaded into the finished arena so that
    /// `Value::Str` ids emitted during construction stay resolvable (Gap G3).
    strings: StringTable,
}

impl ArenaBuilder {
    /// Creates a builder with an empty arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Packs `node`, returning its ID.
    pub fn pack(&mut self, node: Node) -> NodeId {
        self.arena.pack(node)
    }

    /// Registers `closure` in the arena's closure table.
    pub fn add_closure(&mut self, closure: ClosureIR) {
        self.arena.add_closure(closure);
    }

    /// Interns `text`, returning its [`StringId`].
    ///
    /// The accumulated table is moved into the finished [`IRArena`] by
    /// [`finish`](Self::finish), so identifiers produced here resolve from
    /// `IRArena::string_table`.
    pub fn intern_string(&mut self, text: &str) -> StringId {
        self.strings.intern(text)
    }

    /// Returns a read-only view of a packed node.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<NodeView<'_>> {
        self.arena.get(id)
    }

    /// Attaches a node's ADR-0027 Phase 2/3 signal-graph metadata (T13/T14)
    /// after packing it: its `signal_deps`, optional `prop_thunk` closure
    /// reference, and `prop_layout`. See [`IRArena::set_signal_metadata`].
    pub fn signal_metadata(
        &mut self,
        id: NodeId,
        deps: Vec<flux_syntax::SignalId>,
        thunk: Option<flux_syntax::ClosureRef>,
        layout: Vec<u16>,
    ) {
        self.arena.set_signal_metadata(id, deps, thunk, layout);
    }

    /// Consumes the builder, yielding the packed [`IRArena`].
    ///
    /// The interner accumulated via [`intern_string`](Self::intern_string) is
    /// moved into the arena, closing Gap G3.
    #[must_use]
    pub fn finish(self) -> IRArena {
        self.arena.with_string_table(self.strings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_id::compute_node_id;
    use flux_syntax::{PropIdx, Value};

    #[test]
    fn builder_packs_and_reads_back() {
        let mut b = ArenaBuilder::new();
        let id = compute_node_id(0, NodeKind::Primitive, Span::new(0, 0, 3), None);
        b.pack(Node {
            id,
            kind: NodeKind::Primitive,
            component_id: 2,
            props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(1))]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 3),
        });
        let arena = b.finish();
        let view = arena.get(id).unwrap();
        assert_eq!(view.kind(), NodeKind::Primitive);
        assert_eq!(view.props().get(PropIdx::from(0u16)), Some(&Value::Int(1)));
    }

    #[test]
    fn builder_stores_closures() {
        let mut b = ArenaBuilder::new();
        let closure = ClosureIR::new(
            HandlerId::from(1u32),
            vec![0x00],
            vec![],
            Span::new(0, 0, 1),
        );
        b.add_closure(closure);
        assert!(b.finish().closure(HandlerId::from(1u32)).is_some());
    }
}

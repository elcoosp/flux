//! Nodes, node kinds and props — the shape of the reactive tree
//! (Appendix C §C.1).

use crate::ids::{ComponentId, HandlerId, Key, NodeId, PropIdx, Span};
use crate::strings::StringTable;
use crate::value::Value;

/// The kind of an IR node.
///
/// The discriminants are normative on the wire (Appendix D §D.3).
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub enum NodeKind {
    /// A user-defined or stdlib component invocation.
    Component = 0,
    /// A leaf primitive backed directly by an adapter.
    Primitive = 1,
    /// A keyed list.
    ForEach = 2,
    /// A conditional subtree (`when` / `if`).
    If = 3,
    /// A pattern match over an algebraic data type.
    Match = 4,
    /// A navigation stack.
    Router = 5,
    /// One screen within a [`NodeKind::Router`].
    Screen = 6,
}

impl NodeKind {
    /// Every kind, in wire-tag order.
    pub const ALL: [Self; 7] = [
        Self::Component,
        Self::Primitive,
        Self::ForEach,
        Self::If,
        Self::Match,
        Self::Router,
        Self::Screen,
    ];

    /// Returns the wire tag for this kind.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Parses a wire tag, returning `None` for an unknown value.
    ///
    /// A total function is used rather than a transmute so that a corrupt or
    /// future-versioned frame is reported as a protocol error instead of
    /// producing an invalid enum value.
    #[must_use]
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Component),
            1 => Some(Self::Primitive),
            2 => Some(Self::ForEach),
            3 => Some(Self::If),
            4 => Some(Self::Match),
            5 => Some(Self::Router),
            6 => Some(Self::Screen),
            _ => None,
        }
    }
}

/// A flat, order-independent prop map for one node.
///
/// Props are content-addressed: [`Props::hash`] is a BLAKE3-derived digest used
/// to skip re-sending unchanged props over the wire (Appendix D §D.14).
///
/// # Examples
///
/// ```
/// use flux_syntax::{Props, Value};
///
/// let props = Props::from_fields(vec![(0, Value::Int(12))]);
/// assert_eq!(props.get(0), Some(&Value::Int(12)));
/// ```
#[derive(Clone, Debug)]
pub struct Props {
    fields: Vec<(PropIdx, Value)>,
    hash: u64,
}

impl Default for Props {
    /// An empty prop map, hashed identically to `Props::from_fields(vec![])`.
    fn default() -> Self {
        Self::from_fields(Vec::new())
    }
}

impl Props {
    /// Builds a prop map from `fields` and computes its content hash.
    #[must_use]
    pub fn from_fields(fields: Vec<(PropIdx, Value)>) -> Self {
        let hash = hash_fields(&fields);
        Self { fields, hash }
    }

    /// Returns the fields in insertion order.
    #[must_use]
    pub fn fields(&self) -> &[(PropIdx, Value)] {
        &self.fields
    }

    /// Returns the content hash of this prop map.
    #[must_use]
    pub const fn hash(&self) -> u64 {
        self.hash
    }

    /// Returns `true` when no props are set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Returns the number of set props.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Looks up the value stored at `index`.
    #[must_use]
    pub fn get(&self, index: PropIdx) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(candidate, _)| *candidate == index)
            .map(|(_, value)| value)
    }

    /// Resolves the string prop at `index` through `table`.
    ///
    /// Returns `None` when the prop is absent, is not a string, or names an ID
    /// that `table` does not know.
    #[must_use]
    pub fn get_str<'a>(&self, index: PropIdx, table: &'a StringTable) -> Option<&'a str> {
        table.resolve(self.get(index)?.as_str_id()?)
    }

    /// Reads the boolean prop at `index`, falling back to `default` when the
    /// prop is absent or is not a boolean.
    #[must_use]
    pub fn get_bool(&self, index: PropIdx, default: bool) -> bool {
        self.get(index).and_then(Value::as_bool).unwrap_or(default)
    }

    /// Reads the integer prop at `index`.
    #[must_use]
    pub fn get_int(&self, index: PropIdx) -> Option<i64> {
        self.get(index).and_then(Value::as_int)
    }

    /// Reads the float prop at `index`.
    #[must_use]
    pub fn get_float(&self, index: PropIdx) -> Option<f64> {
        self.get(index).and_then(Value::as_float)
    }

    /// Reads the handler bound to `index`.
    ///
    /// Returns `None` rather than a sentinel ID so that an unbound event is
    /// distinguishable from a handler that legitimately has ID zero.
    #[must_use]
    pub fn get_handler(&self, index: PropIdx) -> Option<HandlerId> {
        self.get(index).and_then(Value::as_handler)
    }
}

/// Hashes a field set order-independently by XOR-folding per-field digests.
fn hash_fields(fields: &[(PropIdx, Value)]) -> u64 {
    let mut accumulator: u64 = 0xcbf2_9ce4_8422_2325;
    for (index, value) in fields {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&index.to_le_bytes());
        value.hash_into(&mut hasher);
        let mut digest = [0_u8; 8];
        digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        accumulator ^= u64::from_le_bytes(digest);
    }
    accumulator
}

/// One child slot of a node.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Child {
    /// A single statically known child.
    Node(NodeId),
    /// A dynamic, keyed run of children produced by a `ForEach`.
    Splice {
        /// `(key, node)` pairs in render order.
        items: Vec<(Key, NodeId)>,
    },
}

impl Child {
    /// Iterates over the node IDs this slot contributes, in render order.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        let single = match self {
            Self::Node(id) => Some(*id),
            Self::Splice { .. } => None,
        };
        let spliced = match self {
            Self::Node(_) => None,
            Self::Splice { items } => Some(items.iter().map(|(_, id)| *id)),
        };
        single.into_iter().chain(spliced.into_iter().flatten())
    }
}

/// A fully materialised IR node.
#[derive(Clone, Debug)]
pub struct NodeRef {
    /// Stable identity, derived from source structure.
    pub id: NodeId,
    /// Node kind.
    pub kind: NodeKind,
    /// Interned name of the component or primitive being invoked.
    pub component_id: ComponentId,
    /// Flat prop map.
    pub props: Props,
    /// Child slots in render order.
    pub children: Vec<Child>,
    /// Handlers bound by this node.
    pub handlers: Vec<HandlerId>,
    /// Source span this node was lowered from.
    pub span: Span,
}

impl NodeRef {
    /// Iterates over every child node ID, flattening splices.
    pub fn child_node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.children.iter().flat_map(Child::node_ids)
    }

    /// Returns `true` when this node has no children.
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.child_node_ids().next().is_none()
    }
}

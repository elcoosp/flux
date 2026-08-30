//! Identifier aliases and source spans (Appendix C §C.1).

pub use fnv::{
    ComponentId, EffectId, FileId, HandlerId, InstanceId, Key, NodeId, PropIdx, SignalId, StringId,
    TypeId, fnv1a32,
};
pub use node_id::{compute_node_id, content_addressed_id};
pub use node_tag::{DeclTag, ExprTag, NodeTag};
pub use span::{SourceExcerpt, Span};

mod fnv;
mod node_id;
mod node_tag;
mod span;

#[cfg(test)]
mod tests;

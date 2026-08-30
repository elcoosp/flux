//! Shared type vocabulary for the Flux toolchain.
//!
//! Every crate in the workspace speaks in these types: the parser, type
//! checker, IR, differ, serializer, dev server and both codegen backends.
//! Nothing here depends on any other Flux crate, which is what makes it safe
//! for parallel development (see `/docs/agents-boundaries-contract.md` §1.4).
//!
//! The definitions are normative against Appendix C (IR schema) and Appendix D
//! (wire protocol) of `/docs/spec/mlp-appendices.md`.
//!
//! # Examples
//!
//! ```
//! use flux_syntax::{Props, StringTable, Value};
//!
//! let mut table = StringTable::new();
//! let label = table.intern("Increment");
//! let props = Props::from_fields(vec![(0, Value::Str(label))]);
//!
//! assert_eq!(props.get_str(0, &table), Some("Increment"));
//! ```

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod ids;
mod node;
pub mod opcode;
mod patch;
mod strings;
mod ty;
mod value;

pub use ids::{
    ComponentId, DeclTag, EffectId, ExprTag, FileId, HandlerId, InstanceId, Key, NodeId, NodeTag,
    PropIdx, SignalId, SourceExcerpt, Span, StringId, TypeId, compute_node_id,
    content_addressed_id, fnv1a32,
};
pub use node::{Child, NodeKind, NodeRef, Props};
pub use patch::{ClosureRef, Patch, PropDiff};
pub use strings::StringTable;
pub use ty::TypeKind;
pub use value::Value;

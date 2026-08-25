//! `flux-ir` — the reactive-tree IR core (Appendix C §C.1, FLUX-004).
//!
//! This crate owns the in-memory shape of a lowered Flux UI: a packed
//! [`IRArena`] of nodes, the [`ClosureIR`] bytecode table, and the
//! [`InstanceRegistry`] that lets the host app preserve state across hot swaps.
//! Node identities are derived once, stably, from source structure via
//! [`compute_node_id`] (ADR-0013) so that diffing and state preservation work.
//!
//! Lowering (`.flux` → this IR) is FLUX-018 and lives in the `lower` module
//! of this crate (it is an `flux-ir` extension, owned by the same directory as
//! the arena core). This crate provides the data structures
//! ([`IRArena`], [`ClosureIR`], [`InstanceRegistry`]), the stable-ID
//! derivation ([`compute_node_id`]), the hand-construction [`ArenaBuilder`]
//! API, and the [`lower::lower`] pass that turns a type-checked program into the
//! packed reactive tree the differ and wire codec consume.
//!
//! # Examples
//!
//! ```
//! use flux_ir::{ArenaBuilder, Node, compute_node_id};
//! use flux_syntax::{NodeKind, Props, Span};
//!
//! let mut builder = ArenaBuilder::new();
//! let id = compute_node_id(0, NodeKind::Component, Span::new(0, 0, 4), None);
//! builder.pack(Node {
//!     id,
//!     kind: NodeKind::Component,
//!     component_id: 1,
//!     props: Props::default(),
//!     children: vec![],
//!     handlers: vec![],
//!     span: Span::new(0, 0, 4),
//! });
//! let arena = builder.finish();
//! assert!(arena.get(id).is_some());
//! ```
#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod arena;
mod builder;
mod closure;
mod instance;
pub mod lower;
mod node_id;

pub use arena::{IRArena, NodeView};
pub use builder::{ArenaBuilder, Node};
pub use closure::ClosureIR;
pub use instance::{ComponentInstance, InstanceRegistry};
pub use lower::bytecode::{HandlerCompileError, compile_handler};
pub use lower::prop_index_for_name;
pub use lower::{LoweredIr, LoweringError, lower};
pub use node_id::compute_node_id;

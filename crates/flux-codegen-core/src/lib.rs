//! `flux-codegen-core` — the shared, data-driven code generation layer for Flux
//! release backends (FLUX-047).
//!
//! Both `flux-codegen-kotlin` and `flux-codegen-swift` previously duplicated the
//! entire per-node emitter (`emit_primitive`, `emit_if`, `emit_for_each`,
//! `render_button_label`, `collect_handler`, …) and the expression renderer.
//! This crate extracts the shared ~80% — structural traversal of the lowered
//! tree, the primitive registry, the node-ID bridge, and expression rendering —
//! and leaves only the genuinely language-specific syntax to a [`Backend`]
//! trait. Each backend crate becomes a thin `Backend` impl plus its component
//! header (`@Composable fun` vs `struct …: View`).
//!
//! The primitive registry ([`primitives`]) is the single source of truth,
//! mirroring the project's capability-IDL pattern
//! (`flux_types::capabilities`): one declarative table, two backends reading it,
//! and a parity guard (the `parity` test module) that fails if the table drifts
//! from what the prelude registers.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

pub mod backend;
pub mod bridge;
pub mod emitter;
pub mod error;
pub mod expressions;
pub mod model;
pub mod native_gen;
pub mod primitives;
pub mod view_tree;

#[cfg(test)]
mod parity;

pub use backend::Backend;
pub use bridge::Bridge;
pub use emitter::Emitter;
pub use error::CodegenError;
pub use primitives::PrimitiveSpec;
pub use view_tree::{ViewNode, view_tree};

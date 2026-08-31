//! The type checker's internal type representation.
//!
//! [`TypeKind`] from `flux-syntax` is the *serialised* structural type used by
//! the IR, differ and codegen. During checking we need a richer form that can
//! carry unresolved names ([`TcType::Named`]) before we resolve them to ADTs or
//! type constructors, as well as unification variables. [`TcType`] is that form;
//! [`TcType::to_typekind`] converts back to [`TypeKind`] once a type is fully
//! resolved and concrete.

pub use tc_type::TcType;

pub(crate) use node_id::{compute_node_id, decl_tag};

mod node_id;
mod tc_type;

#[cfg(test)]
mod tests;

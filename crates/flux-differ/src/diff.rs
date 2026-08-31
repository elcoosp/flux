//! Keyed tree differencing between two lowered arenas (FLUX-014).
//!
//! [`diff`] consumes the old and new [`IRArena`](flux_ir::IRArena)s and produces
//! a [`Patch`](flux_syntax::Patch) vector that the host applies to keep its
//! shadow tree state-preserving: nodes that survive an edit (same construct,
//! parent, and slot) are `Reattach`ed rather than removed and re-inserted, so
//! scroll position, focus, and animation state live across hot reload.

pub use algorithm::diff;

mod algorithm;
mod compare;
mod emit;
mod tree;

#[cfg(test)]
mod tests;

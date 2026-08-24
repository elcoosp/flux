//! `flux-differ` — keyed tree differencing for the Flux reactive tree (FLUX-014).
//!
//! Produces the minimal [`Patch`] stream that transforms an old [`IRArena`]
//! into a new one, reconciling over the stable [`NodeId`]s derived by
//! `flux_ir::compute_node_id`. This is the unit of hot-swap shipped over the
//! wire (Appendix D) and applied by the host runtimes.
//!
//! # Algorithm
//!
//! `diff` performs udomdiff-style keyed reconciliation:
//! - nodes present in both arenas are compared; structural changes emit
//!   [`Patch::Replace`], prop-only changes emit [`Patch::Update`], and
//!   handler-body changes emit [`Patch::Handler`] (the state-preserving path);
//! - a parent whose child *set* is unchanged but whose order differs emits a
//!   single [`Patch::Reorder`] rather than remove+insert;
//! - nodes missing from the new arena emit [`Patch::Remove`];
//! - nodes new to the new arena emit [`Patch::Insert`] with their parent and
//!   insertion index.
//!
//! [`NodeId`]: flux_syntax::NodeId
//! [`IRArena`]: flux_ir::IRArena
//! [`Patch`]: flux_syntax::Patch
//! [`Patch::Replace`]: flux_syntax::Patch::Replace
//! [`Patch::Update`]: flux_syntax::Patch::Update
//! [`Patch::Handler`]: flux_syntax::Patch::Handler
//! [`Patch::Reorder`]: flux_syntax::Patch::Reorder
//! [`Patch::Remove`]: flux_syntax::Patch::Remove
//! [`Patch::Insert`]: flux_syntax::Patch::Insert

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod diff;

pub use diff::diff;

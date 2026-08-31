//! Go-to-definition: maps a cursor position to the declaring span of the
//! symbol under it.
//!
//! The provider is a pure function over the parsed [`Ast`] (no type-checking,
//! no lowering) so it can be unit-tested without a socket. It builds an index
//! of every *binding site* in the file — top-level declarations, component
//! props, function/lambda parameters, `state` cells, `let`/`match` bindings,
//! `ForEach` key closures — and resolves a cursor by extracting the identifier
//! word at that offset, then picking the declaration whose scope contains the
//! cursor and whose name matches, preferring the tightest enclosing scope so
//! an inner binding correctly shadows an outer one.

pub(crate) use index::DefIndex;

mod index;

#[cfg(test)]
mod tests;

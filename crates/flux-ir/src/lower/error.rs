//! Lowering error type (FLUX-018).
//!
//! Every lowering failure carries the source [`Span`] it occurred at, following
//! the diagnostic contract in AGENTS.md §3.7 (what / where / why / how).
//! Lowering never panics on malformed-but-typed input; it reports.

use flux_syntax::Span;
use thiserror::Error;

/// An error produced while lowering a typed AST into the reactive-tree IR.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum LoweringError {
    /// A construct could not be lowered; `message` explains why, `span` points
    /// at the offending source.
    #[error("lowering error at {span:?}: {message}")]
    Lower {
        /// Human-readable cause.
        message: String,
        /// Source span of the offending construct.
        span: Span,
    },
}

impl LoweringError {
    /// Constructs a lowering error with `message` at `span`.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self::Lower {
            message: message.into(),
            span,
        }
    }
}

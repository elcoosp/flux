//! Codegen error type (FLUX-021).
//!
//! Every codegen failure carries the source [`Span`] it occurred at, following
//! the diagnostic contract in AGENTS.md §3.7 (what / where / why / how).
//! `codegen` itself never panics on well-formed input — it renders best-effort
//! Compose and only returns a [`CodegenError`] when a construct cannot be
//! represented at all.

use flux_syntax::Span;
use thiserror::Error;

/// An error produced while generating Kotlin/Compose from a lowered Flux program.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum CodegenError {
    /// A construct could not be codegen'd; `message` explains why, `span`
    /// points at the offending source.
    #[error("codegen error at {span:?}: {message}")]
    Lower {
        /// Human-readable cause.
        message: String,
        /// Source span of the offending construct.
        span: Span,
    },
}

impl CodegenError {
    /// Constructs a codegen error with `message` at `span`.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self::Lower {
            message: message.into(),
            span,
        }
    }
}

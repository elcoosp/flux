//! Diagnostic types for the Flux type checker.
//!
//! Every error carries a [`Span`] (the "where"), a `message` (the "what"), an
//! optional `hint` (the "how"), and — when the failure is a type mismatch — the
//! `expected` and `actual` types (the "why"). This follows the diagnostic
//! contract in `AGENTS.md` S3.11.
//!
//! On top of the per-phase errors ([`TypeError`] and the parse / lowering /
//! runtime faults), this module defines the unified [`FluxError`] umbrella: the
//! single shape the dev server and host runtimes emit, always with a
//! `what`/`where`/`why`/`how` payload. `LoweringError` lives in `flux-ir`, which
//! depends on `flux-types`, so it is represented via [`FluxError::Compile`] with
//! [`CompilePhase::Lower`] (built through [`CompileError::from_lowering`]) rather
//! than a `From` impl that would form a dependency cycle.

pub use capability_error::CapabilityError;
pub use compile_err::{CompileError, CompilePhase};
pub use constructors::{capability_denied, compile_error};
pub use flux_error::FluxError;
pub use runtime_error::RuntimeError;
pub use type_error::TypeError;

pub(crate) use type_error::line_col;

mod capability_error;
mod compile_err;
mod constructors;
mod flux_error;
mod runtime_error;
mod type_error;

#[cfg(test)]
mod tests;

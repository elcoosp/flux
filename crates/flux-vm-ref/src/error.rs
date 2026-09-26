//! Error types for the reference VM.
//!
//! Every error carries an optional [`Span`] so diagnostics can point at the
//! offending instruction. The error kinds are a superset of Appendix E §E.6:
//! `DivByZero` is added by ADR-0023 (integer division by zero must fail rather
//! than panic) and `NullDereference` vs `TypeMismatch` for `GET_FIELD` is
//! resolved by ADR-0024.

use flux_syntax::Span;
use thiserror::Error;

/// Why a handler invocation terminated without producing a value.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum VmErrorKind {
    /// The 100,000-instruction gas budget was exhausted (Appendix E §E.3).
    #[error("gas budget exhausted")]
    GasExhausted,
    /// The 16 MiB frame memory pool was exhausted.
    #[error("memory pool exhausted")]
    MemoryExhausted,
    /// An index (list/record/string) fell outside its bounds.
    #[error("index out of bounds")]
    IndexOutOfBounds,
    /// A field access was performed on `Null` (ADR-0024).
    #[error("null dereference")]
    NullDereference,
    /// The dispatch byte was not a valid opcode.
    #[error("invalid opcode dispatch")]
    InvalidDispatch,
    /// Operand types were not what the (monomorphized) opcode expected.
    #[error("type mismatch")]
    TypeMismatch,
    /// Integer division or remainder by zero (ADR-0023).
    #[error("division by zero")]
    DivByZero,
}

/// A VM fault with its location in the bytecode.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("{kind} at offset {offset}")]
pub struct VmError {
    /// The category of fault.
    pub kind: VmErrorKind,
    /// Byte offset of the offending instruction in the program.
    pub offset: u32,
    /// Source span, when the handler was lowered from `.flux`.
    pub span: Option<Span>,
}

impl VmError {
    /// Constructs an error located at `offset`, with no source span.
    #[must_use]
    pub const fn at(kind: VmErrorKind, offset: u32) -> Self {
        Self {
            kind,
            offset,
            span: None,
        }
    }
}

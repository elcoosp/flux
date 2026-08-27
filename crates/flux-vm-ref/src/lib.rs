//! Reference implementation of the Flux VM, used as a test oracle only.
//!
//! The bytecode semantics encoded here are the behavioral source of truth for the
//! instruction set in Appendix E. The production runtimes (Swift `FluxBytecodeVM`,
//! Kotlin `FluxBytecodeVM`) and `flux-vm-ref` itself are all validated against
//! the golden ISA vectors under `/tests/isa-vectors/`. Because this crate is the
//! oracle, it is intentionally dependency-light: it decodes Appendix E bytecode
//! with [`decode_program`], interprets it with [`run`], and reports faults via
//! [`VmError`].
//!
//! # Examples
//!
//! ```rust
//! use flux_vm_ref::{run, InMemorySignals};
//! use flux_syntax::Value;
//!
//! // LOAD_INT_CONST r0, 7 ; HALT. Gas is charged once (HALT is free per ADR-0021).
//! let prog = [0xB0, 0, 7, 0, 0, 0, 0, 0, 0, 0,   // LOAD_INT_CONST r0, 7
//!              0x00];                              // HALT
//! let mut signals = InMemorySignals::default();
//! let out = run(&prog, &mut signals, Value::Null).unwrap();
//! assert_eq!(out.gas_used, 1);
//! assert_eq!(out.registers[0], Value::Int(7));
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, unreachable_pub)]

mod decode;
mod error;
mod vm;

pub use decode::{Instruction, decode_program};
pub use error::{VmError, VmErrorKind};
pub use vm::{
    InMemorySignals, RunResult, SignalStore, SuspendState, VmOutcome, resume, run, run_resumable,
};

#[cfg(test)]
mod tests {
    use super::*;
    use flux_syntax::Value;

    #[test]
    fn gas_counts_non_halt_only() {
        let prog = [0x01, 0xB0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0x00]; // NOP + LOAD + HALT
        let out = run(&prog, &mut InMemorySignals::default(), Value::Null).unwrap();
        assert_eq!(out.gas_used, 2);
    }

    #[test]
    fn invalid_dispatch_errors() {
        let err = run(&[0xFF], &mut InMemorySignals::default(), Value::Null).unwrap_err();
        assert_eq!(err.kind, VmErrorKind::InvalidDispatch);
        assert_eq!(err.offset, 0);
    }

    #[test]
    fn truncated_program_errors() {
        // LOAD_INT_CONST needs 9 operand bytes; supply only 3.
        let err = run(
            &[0xB0, 0, 1, 2],
            &mut InMemorySignals::default(),
            Value::Null,
        )
        .unwrap_err();
        assert_eq!(err.kind, VmErrorKind::IndexOutOfBounds);
    }

    #[test]
    fn int_div_by_zero_is_divbyzero() {
        let prog = [
            0xB0, 0, 1, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r0, 1
            0xB0, 1, 0, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r1, 0
            0x23, 2, 0, 1, // DIV_I64 r2, r0, r1
            0x00,
        ];
        let err = run(&prog, &mut InMemorySignals::default(), Value::Null).unwrap_err();
        assert_eq!(err.kind, VmErrorKind::DivByZero);
    }

    #[test]
    fn float_div_by_zero_is_inf() {
        let prog = [
            0xB1, 0, 0, 0, 0, 0, 0, 0, 0xF8, 0x3F, // LOAD_FLOAT_CONST r0, 1.5
            0xB1, 1, 0, 0, 0, 0, 0, 0, 0, 0x00, // LOAD_FLOAT_CONST r1, 0.0
            0x33, 2, 0, 1, // DIV_F64 r2, r0, r1
            0x00,
        ];
        let out = run(&prog, &mut InMemorySignals::default(), Value::Null).unwrap();
        assert_eq!(out.registers[2], Value::Float(f64::INFINITY));
    }

    #[test]
    fn get_field_on_null_is_null_deref() {
        let prog = [
            0xB4, 0, // LOAD_NULL r0
            0x71, 1, 0, 0, 0, // GET_FIELD r1, r0, 0
            0x00,
        ];
        let err = run(&prog, &mut InMemorySignals::default(), Value::Null).unwrap_err();
        assert_eq!(err.kind, VmErrorKind::NullDereference);
    }
}

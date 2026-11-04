//! Round-21 regressions for the reference VM: crafted bytecode and crafted
//! suspend states must surface as `VmError`s, never panics (fuzz charter:
//! attacker-controlled bytes must NEVER panic the oracle).

use flux_syntax::{StringTable, Value};
use flux_vm_ref::{
    InMemorySignals, RunResult, SuspendState, VmErrorKind, resume, run, run_resumable,
};

fn load_int_const(dst: u8, n: i64) -> Vec<u8> {
    let mut v = vec![flux_syntax::opcode::raw::LOAD_INT_CONST, dst];
    v.extend_from_slice(&n.to_le_bytes());
    v
}

/// `LOAD_NULL r16` — a register operand indexing one past the 16-register file.
/// Pre-fix this panicked with "index out of bounds: the len is 16".
#[test]
fn out_of_range_register_operand_is_a_vm_error_not_a_panic() {
    let err = run(
        &[flux_syntax::opcode::raw::LOAD_NULL, 0x10, 0x00],
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Null,
    )
    .expect_err("r16 must be rejected");
    assert_eq!(err.kind, VmErrorKind::IndexOutOfBounds);
    assert_eq!(err.offset, 0);
}

/// `MOV r200, r0` — same class, larger index.
#[test]
fn mov_with_out_of_range_registers_is_rejected() {
    let result = run(
        &[flux_syntax::opcode::raw::MOV, 0xC8, 0x00],
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Null,
    );
    assert!(result.is_err(), "r200 must be rejected, got {result:?}");
}

/// A register operand beyond the operand window (e.g. `EQ_I64` reading what is
/// actually the next opcode byte) cannot crash either — the misalignment bug
/// class is closed at the decode layer, so only in-window bytes are validated;
/// the VM still returns errors for type misuse rather than panicking.
#[test]
fn valid_programs_still_run_after_register_validation() {
    let mut code = load_int_const(0, 7);
    code.push(flux_syntax::opcode::raw::HALT);
    let out = run(
        &code,
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Null,
    )
    .expect("valid program must run");
    assert_eq!(out.registers[0], Value::Int(7));
}

/// A crafted `SuspendState` naming a register ≥ 16 as `result_reg` used to
/// panic when `resume` wrote the delivered value.
#[test]
fn resume_with_crafted_result_reg_is_rejected() {
    let state = SuspendState {
        program: vec![flux_syntax::opcode::raw::HALT],
        resume_ip: 0,
        registers: std::array::from_fn(|_| Value::Null),
        gas_remaining: 100,
        signals: vec![],
        future_reg: 0,
        result_reg: 255,
    };
    let err = resume(
        state,
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Int(1),
    )
    .expect_err("result_reg 255 must be rejected");
    assert_eq!(err.kind, VmErrorKind::IndexOutOfBounds);
}

/// A crafted `SuspendState` claiming more remaining gas than a fresh entry
/// used to underflow `ENTRY_GAS - gas` in `finish` (debug panic).
#[test]
fn resume_with_inflated_gas_is_rejected() {
    let state = SuspendState {
        program: vec![flux_syntax::opcode::raw::HALT],
        resume_ip: 0,
        registers: std::array::from_fn(|_| Value::Null),
        gas_remaining: u32::MAX,
        signals: vec![],
        future_reg: 0,
        result_reg: 0,
    };
    let err = resume(
        state,
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Null,
    )
    .expect_err("inflated gas must be rejected");
    assert_eq!(err.kind, VmErrorKind::GasExhausted);
}

/// A well-formed suspension still resumes end-to-end after the new validation:
/// `resume_ip` pointing at HALT, value delivered into `result_reg`.
#[test]
fn legitimate_resume_still_works() {
    let state = SuspendState {
        program: vec![flux_syntax::opcode::raw::HALT],
        resume_ip: 0,
        registers: std::array::from_fn(|_| Value::Null),
        gas_remaining: 100,
        signals: vec![],
        future_reg: 1,
        result_reg: 0,
    };
    let out = resume(
        state,
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Int(5),
    )
    .expect("legitimate resume must work");
    match out {
        RunResult::Halt(outcome) => {
            assert_eq!(outcome.registers[0], Value::Int(5));
            // gas_used is measured against the full 100_000 ENTRY_GAS budget,
            // not the resumed state's remaining gas.
            assert!(outcome.gas_used <= 100_000);
        }
        other => panic!("expected Halt, got {other:?}"),
    }
}

/// Fuzz smoke: random bytecode blobs never panic the `run`/`run_resumable`
/// entry points (the fuzz suite only covered the serde decoders before).
#[test]
fn random_bytecode_never_panics() {
    // Deterministic LCG so failures reproduce.
    let mut seed: u64 = 0x5EED;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as u8
    };
    for _ in 0..500 {
        let len = (next() as usize % 24) + 1;
        let blob: Vec<u8> = (0..len).map(|_| next()).collect();
        let mut signals = InMemorySignals::default();
        // Any outcome is fine — panic is the only failure.
        let _ = run(&blob, &mut signals, &StringTable::new(), Value::Null);
        let mut signals = InMemorySignals::default();
        let _ = run_resumable(&blob, &mut signals, &StringTable::new(), Value::Null);
    }
}

//! Round-21 adversarial tests for the reference VM: *structured* corruption of
//! a valid program. Complements `round21_register_validation.rs` (register
//! operand ranges) and its random-blob fuzz (which rarely produces *decodable*
//! instruction streams) with exhaustive truncation and per-byte corruption of
//! a real multi-instruction program.
//!
//! Contract: for every truncation and every single-byte corruption, `run` and
//! `run_resumable` return Ok or a `VmError` — never panic, never execute past
//! the gas budget.

use flux_syntax::opcode::raw;
use flux_syntax::{StringTable, Value};
use flux_vm_ref::{InMemorySignals, RunResult, run, run_resumable};

/// LOAD_INT_CONST r0, 7 ; CALL_CAP r1, cap 1 method 1, args r0 ; AWAIT r2, r1
/// ; WRITE_SIGNAL sig 2, r0 ; HALT — a realistic async handler shape.
fn valid_program() -> Vec<u8> {
    let mut prog = Vec::new();
    // LOAD_INT_CONST r0, 7 (opcode + dst + i64 LE)
    prog.push(raw::LOAD_INT_CONST);
    prog.push(0u8);
    prog.extend_from_slice(&7i64.to_le_bytes());
    // CALL_CAP dst_reg(u8)=1, cap_id(u32)=1, method_id(u16)=1, args_reg(u8)=0
    prog.push(raw::CALL_CAP);
    prog.push(1u8);
    prog.extend_from_slice(&1u32.to_le_bytes());
    prog.extend_from_slice(&1u16.to_le_bytes());
    prog.push(0u8);
    // AWAIT result_reg(u8)=2, future_reg(u8)=1
    prog.push(raw::AWAIT);
    prog.push(2u8);
    prog.push(1u8);
    // WRITE_SIGNAL sig_id(u32)=2, src_reg(u8)=2
    prog.push(raw::WRITE_SIGNAL);
    prog.extend_from_slice(&2u32.to_le_bytes());
    prog.push(2u8);
    // HALT
    prog.push(raw::HALT);
    prog
}

/// Sanity: the uncorrupted program runs under `run_resumable` (it suspends on
/// the unregistered async cap, or halts via the parity stub — either way the
/// pipeline is live before we start corrupting it).
#[test]
fn valid_program_is_live_before_corruption() {
    let prog = valid_program();
    let mut signals = InMemorySignals::default();
    let out = run_resumable(&prog, &mut signals, &StringTable::new(), Value::Null)
        .expect("valid program must not fault");
    assert!(matches!(out, RunResult::Suspended(_) | RunResult::Halt(_)));
}

/// Truncating a valid program at EVERY length must produce a clean `VmError`
/// (or, for a still-valid prefix, a normal outcome) — never a panic.
#[test]
fn every_truncation_is_clean() {
    let prog = valid_program();
    for cut in 0..prog.len() {
        let truncated = &prog[..cut];
        let mut signals = InMemorySignals::default();
        let _ = run(truncated, &mut signals, &StringTable::new(), Value::Null);
        let mut signals = InMemorySignals::default();
        let _ = run_resumable(truncated, &mut signals, &StringTable::new(), Value::Null);
    }
}

/// Flipping any single byte to 0x00 and 0xFF must never panic. Corrupted
/// operands (registers, signal ids, jump targets, string ids) must fault
/// through validation, not crash the interpreter.
#[test]
fn every_single_byte_flip_is_clean() {
    let prog = valid_program();
    for pos in 0..prog.len() {
        for &fill in &[0x00u8, 0xFFu8] {
            let mut corrupted = prog.clone();
            corrupted[pos] = fill;
            let mut signals = InMemorySignals::default();
            let _ = run(&corrupted, &mut signals, &StringTable::new(), Value::Null);
            let mut signals = InMemorySignals::default();
            let _ = run_resumable(&corrupted, &mut signals, &StringTable::new(), Value::Null);
        }
    }
}

/// Junk appended after HALT must be ignored or faulted cleanly, and repeated
/// HALTs / deep NOP sleds must stay within the gas budget.
#[test]
fn appended_junk_and_sleds_stay_bounded() {
    let prog = valid_program();
    for extra in [vec![raw::HALT], vec![0xFF; 64], vec![raw::NOP; 256]] {
        let mut extended = prog.clone();
        extended.extend_from_slice(&extra);
        let mut signals = InMemorySignals::default();
        if let Ok(RunResult::Halt(outcome)) =
            run_resumable(&extended, &mut signals, &StringTable::new(), Value::Null)
        {
            assert!(
                outcome.gas_used <= 100_000,
                "gas must stay within the entry budget, got {}",
                outcome.gas_used
            );
        }
    }
    // A bare NOP sled of the full gas budget must terminate (not hang).
    let sled = vec![raw::NOP; 120_000];
    let mut signals = InMemorySignals::default();
    match run(&sled, &mut signals, &StringTable::new(), Value::Null) {
        Ok(outcome) => assert!(outcome.gas_used <= 100_000, "gas budget enforced"),
        Err(err) => assert!(!err.to_string().is_empty(), "error carries a message"),
    }
}

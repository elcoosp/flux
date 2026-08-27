//! Conformance test for MLP v2 first-class async + the unified capability bridge
//! (ADR-0044 / ADR-0045).
//!
//! Exercises the suspend/resume behavior of the reference VM together with the
//! `CALL_CAP` → signal-cell → `AWAIT` contract:
//!
//! - `flux_vm_ref::RunResult`      (enum: `Halt(VmOutcome) | Suspended(SuspendState)`)
//! - `flux_vm_ref::SuspendState`    (program, resume_ip, regs[16], gas, captured signals)
//! - `flux_vm_ref::run_resumable`   (returns `RunResult`, suspends on `AWAIT`/0xE0)
//! - `flux_vm_ref::resume`          (re-enters the interpreter at the saved ip)
//!
//! Scenario A — synchronous capability (no real park):
//!   CALL_CAP r2, cap=1, method=1, args=r0   ; r2 = result-cell id (99)
//!   AWAIT   r0, r2                           ; cell 99 is Ready → continue, r0 = value
//!   WRITE_SIGNAL s2, r0                       ; signal 2 = the resolved value
//!   HALT
//! The `AWAIT` does not suspend: a Ready cell continues on the next interpreter
//! re-entry with the value placed in r0 (ADR-0045 §4).
//!
//! Scenario B — asynchronous capability (real suspend):
//!   CALL_CAP r2, cap=2, method=99, args=r0   ; r2 = a fresh Pending cell id
//!   AWAIT   r0, r2                           ; Pending → Suspend
//!   ... host resolves the cell with `resolve_cell` ...
//!   resume(state, value)                     ; cell now Ready → continues, r0 = value
//!   WRITE_SIGNAL s2, r0
//!   HALT
//!
//! The v1 `run` entry point must NOT silently suspend: an `AWAIT` in v1 bytecode
//! is an invalid dispatch, preserving v1 parity.

use flux_syntax::Value;
use flux_vm_ref::{InMemorySignals, RunResult, SignalStore, resume, run, run_resumable};

// CALL_CAP (0x90): opcode, result_reg(u8), cap_id(u32), method_id(u16), args_reg(u8).
// Width per Appendix E §E.1 is 8 payload bytes + 1 opcode = 9 total: [op][r][cap u32][method u16][args u8].
fn call_cap(result_reg: u8, cap_id: u32, method_id: u16, args_reg: u8) -> Vec<u8> {
    let mut b = vec![0x90u8, result_reg];
    b.extend_from_slice(&cap_id.to_le_bytes());
    b.extend_from_slice(&method_id.to_le_bytes());
    b.push(args_reg);
    b
}

#[test]
fn await_suspends_then_resume_completes() {
    // No LOAD needed: the payload (a Record) rides in r0 and is the CALL_CAP argument.
    let load: Vec<u8> = Vec::new();
    // CALL_CAP r2, cap=1, method=1, args=r0  → r2 = 99 (result-cell id)
    let call = call_cap(2, 1, 1, 0);
    // AWAIT r0, r2  (0xE0 + result_reg + future_reg); parks on cell[99]
    let await_op = [0xE0u8, 0, 2];
    // WRITE_SIGNAL s2, r0
    let write2 = [0x11u8, 2, 0, 0, 0, 0];
    // HALT
    let halt = [0x00u8];

    let mut prog: Vec<u8> = Vec::new();
    prog.extend_from_slice(&load);
    prog.extend_from_slice(&call);
    prog.extend_from_slice(&await_op);
    prog.extend_from_slice(&write2);
    prog.extend_from_slice(&halt);

    let mut signals = InMemorySignals::default();

    // v1 `run` must treat an AWAIT as an invalid dispatch: v1 has no suspend concept.
    assert!(
        run(&prog, &mut signals, Value::Null).is_err(),
        "v1 run() must not silently suspend on AWAIT"
    );

    // Scenario A: the parity stub (cap 1/1) writes Ready(value) into cell 99 and
    // returns 99, so AWAIT sees a Ready cell and continues without suspending.
    // First run reaches HALT directly with signal 2 = 42.
    let payload = Value::Record(vec![(0u16, Value::Int(42))]);
    let first = run_resumable(&prog, &mut signals, payload).expect("run_resumable ok");
    let outcome_a = match first {
        RunResult::Halt(o) => o,
        RunResult::Suspended(_) => panic!("sync capability should not suspend on a Ready cell"),
    };
    let written: std::collections::HashMap<u32, Value> = outcome_a.signals.into_iter().collect();
    assert_eq!(
        written.get(&99),
        Some(&Value::Int(42)),
        "capability wrote its argument into signal 99"
    );
    assert_eq!(
        written.get(&2),
        Some(&Value::Int(42)),
        "AWAIT on Ready cell placed the value in r0 → signal 2 = 42"
    );

    // Scenario B: re-run against the reference async capability (cap 2, method 99), which
    // returns a Pending cell. AWAIT on a Pending cell suspends; the host resolves the cell
    // with `resolve_cell`, then `resume` continues with the value in r0.
    let async_call = call_cap(2, 2, 99, 0);
    let mut prog_b: Vec<u8> = Vec::new();
    prog_b.extend_from_slice(&load);
    prog_b.extend_from_slice(&async_call);
    prog_b.extend_from_slice(&await_op);
    prog_b.extend_from_slice(&write2);
    prog_b.extend_from_slice(&halt);

    let mut signals = InMemorySignals::default();
    let payload = Value::Record(vec![(0u16, Value::Int(42))]);
    let first = run_resumable(&prog_b, &mut signals, payload).expect("run_resumable ok (async)");
    let state = match first {
        RunResult::Suspended(s) => s,
        RunResult::Halt(_) => panic!("expected Suspended when the cell is Pending"),
    };
    // CALL_CAP populated r2 with the fresh async cell id.
    let cell_id = match state.registers[2] {
        Value::Int(n) => n as u32,
        _ => panic!("result_reg must hold the cell id"),
    };
    assert!(
        cell_id >= 1_000_001,
        "async capability allocated a fresh cell id"
    );
    // Host resolves the pending cell, then resumes.
    signals.resolve_cell(cell_id, Value::Int(7));
    let resumed = resume(state, &mut signals, Value::Int(7)).expect("resume ok");
    let outcome_b = match resumed {
        RunResult::Halt(o) => o,
        RunResult::Suspended(_) => panic!("expected Halt after resume"),
    };
    let written: std::collections::HashMap<u32, Value> = outcome_b.signals.into_iter().collect();
    assert_eq!(
        written.get(&2),
        Some(&Value::Int(7)),
        "post-resume body ran with the resolved value 7"
    );
}

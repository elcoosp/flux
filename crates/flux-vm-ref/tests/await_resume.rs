//! Conformance test for MLP v2 first-class async (ADR-0044).
//!
//! Exercises the suspend/resume behavior of the reference VM:
//!
//! - `flux_vm_ref::RunResult`        (enum: `Halt(VmOutcome) | Suspended(SuspendState)`)
//! - `flux_vm_ref::SuspendState`      (program, resume_ip, regs[16], gas, captured signals)
//! - `flux_vm_ref::run_resumable`     (returns `RunResult`, suspends on `AWAIT`/0xE0)
//! - `flux_vm_ref::resume`            (re-enters the interpreter at the saved ip)
//!
//! Scenario (ISA vector v2-async-001):
//!   LOAD_INT_CONST r0, 1      ; r0 = 1
//!   WRITE_SIGNAL  s1, r0       ; signal 1 = 1
//!   AWAIT r0, r1              ; suspend; resume value lands in r0
//!   WRITE_SIGNAL  s2, r0       ; signal 2 = resumed value
//!   HALT
//! `run_resumable` returns `Suspended` after the write of signal 1; `resume(state, 42)`
//! continues, writes signal 2 = 42, and returns `Halt` with final signals {1: 1, 2: 42}.
//!
//! The v1 `run` entry point must NOT silently suspend: an `AWAIT` in v1 bytecode is an
//! invalid dispatch, preserving v1 parity.

use flux_syntax::Value;
use flux_vm_ref::{InMemorySignals, RunResult, SignalStore, resume, run, run_resumable};

#[test]
fn await_suspends_then_resume_completes() {
    // LOAD_INT_CONST r0, 1  (9-byte: opcode + reg + i64)
    let load = [0xB0u8, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    // WRITE_SIGNAL s1, r0   (opcode + u32 id + u8 src)
    let write1 = [0x11u8, 1, 0, 0, 0, 0];
    // AWAIT r0, r1          (0xE0 + result_reg + future_reg)
    let await_op = [0xE0u8, 0, 1];
    // WRITE_SIGNAL s2, r0
    let write2 = [0x11u8, 2, 0, 0, 0, 0];
    // HALT
    let halt = [0x00u8];

    let mut prog: Vec<u8> = Vec::new();
    prog.extend_from_slice(&load);
    prog.extend_from_slice(&write1);
    prog.extend_from_slice(&await_op);
    prog.extend_from_slice(&write2);
    prog.extend_from_slice(&halt);

    let mut signals = InMemorySignals::default();

    // v1 `run` must treat an AWAIT as an invalid dispatch: v1 has no suspend concept.
    assert!(
        run(&prog, &mut signals, Value::Null).is_err(),
        "v1 run() must not silently suspend on AWAIT"
    );

    // v2 resumable entry: suspends after writing signal 1.
    let first = run_resumable(&prog, &mut signals, Value::Null).expect("run_resumable ok");
    let state = match first {
        RunResult::Suspended(s) => s,
        RunResult::Halt(_) => panic!("expected Suspended, got Halt on first entry"),
    };
    // Signal 1 was written before the AWAIT.
    assert_eq!(
        signals.read(1),
        Some(Value::Int(1)),
        "signal 1 written before suspend"
    );

    // Resume with the future's resolved value (42).
    let resumed = resume(state, &mut signals, Value::Int(42)).expect("resume ok");
    let outcome = match resumed {
        RunResult::Halt(o) => o,
        RunResult::Suspended(_) => panic!("expected Halt after resume"),
    };
    let written: std::collections::HashMap<u32, Value> = outcome.signals.into_iter().collect();
    assert_eq!(written.get(&1), Some(&Value::Int(1)), "signal 1 retained");
    assert_eq!(
        written.get(&2),
        Some(&Value::Int(42)),
        "signal 2 = resumed value 42"
    );
}

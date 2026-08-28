//! End-to-end async suspension: reference VM ↔ `AwaitSuspend`/`Resume` wire
//! (roadmap Phase 2, ADR-0044 result cells / ADR-0045 capability bridge).
//!
//! The pieces are unit-tested in their own crates: `flux-vm-ref` proves
//! `run_resumable`/`resume`, and `flux-ir-serde` proves the two frames round-trip
//! byte-exactly. What neither can prove alone is that they agree — that the
//! `SuspendState` a real suspended handler produces survives the wire and resumes
//! to the same result it would have reached in-process. That contract is what a
//! host implements against, so it is tested here against the real VM rather than
//! a hand-built state:
//!
//!   run_resumable → Suspended(state) → AwaitSuspend frame → (server settles the
//!   cell) → Resume frame → resume(state, value) → Halt
//!
//! The handler is the async scenario from `flux-vm-ref/tests/await_resume.rs`:
//! `CALL_CAP` against the reference async capability (cap 2, method 99) returns a
//! `Pending` cell, so `AWAIT` genuinely parks.

use flux_ir_serde::{AwaitSuspendFrame, ResumeFrame};
use flux_syntax::Value;
use flux_vm_ref::{InMemorySignals, RunResult, SignalStore, resume, run_resumable};

/// `CALL_CAP` (0x90), 9 bytes: `[op][result_reg][cap u32][method u16][args_reg]`.
fn call_cap(result_reg: u8, cap_id: u32, method_id: u16, args_reg: u8) -> Vec<u8> {
    let mut bytes = vec![0x90u8, result_reg];
    bytes.extend_from_slice(&cap_id.to_le_bytes());
    bytes.extend_from_slice(&method_id.to_le_bytes());
    bytes.push(args_reg);
    bytes
}

/// A handler that awaits the reference *async* capability, so `AWAIT` parks:
/// `CALL_CAP r2, cap=2, method=99, args=r0` · `AWAIT r0, r2` ·
/// `WRITE_SIGNAL s2, r0` · `HALT`.
fn suspending_handler() -> Vec<u8> {
    let mut program = call_cap(2, 2, 99, 0);
    program.extend_from_slice(&[0xE0u8, 0, 2]);
    program.extend_from_slice(&[0x11u8, 2, 0, 0, 0, 0]);
    program.push(0x00u8);
    program
}

/// Runs the handler to its suspension, returning the VM state and the cell id the
/// host would report.
fn suspend_once(signals: &mut InMemorySignals) -> (flux_vm_ref::SuspendState, u32) {
    let program = suspending_handler();
    let payload = Value::Record(vec![(0u16, Value::Int(7))]);
    let state = match run_resumable(&program, signals, payload).expect("handler runs") {
        RunResult::Suspended(state) => state,
        RunResult::Halt(_) => {
            panic!("an async capability must leave the cell Pending, so AWAIT parks")
        }
    };
    let cell = match &state.registers[usize::from(state.future_reg)] {
        Value::Int(id) => u32::try_from(*id).expect("a cell id is a u32"),
        other => panic!("future register must hold the result-cell id, got {other:?}"),
    };
    (state, cell)
}

#[test]
fn a_suspended_handler_reports_the_cell_it_parked_on() {
    let mut signals = InMemorySignals::default();
    let (state, cell) = suspend_once(&mut signals);

    // This is exactly what the host puts on the wire.
    let frame = AwaitSuspendFrame::new(1, cell, state.resume_ip);
    let decoded = AwaitSuspendFrame::from_bytes(&frame.to_bytes()).expect("frame round-trips");

    assert_eq!(
        decoded.cell, cell,
        "the server must learn which cell to settle"
    );
    assert_eq!(
        decoded.resume_ip, state.resume_ip,
        "the resume point must survive the wire so the host re-enters correctly"
    );
}

#[test]
fn a_resume_frame_completes_the_suspended_handler() {
    let mut signals = InMemorySignals::default();
    let (state, cell) = suspend_once(&mut signals);

    // The server settles the cell and ships the resolved value back.
    let resume_frame = ResumeFrame::ready(1, cell, Value::Int(42));
    let delivered =
        ResumeFrame::from_bytes(&resume_frame.to_bytes()).expect("resume frame round-trips");
    assert!(!delivered.is_error, "a Ready cell is not an error");

    signals.resolve_cell(cell, delivered.value.clone());
    let outcome = match resume(state, &mut signals, delivered.value).expect("resumes") {
        RunResult::Halt(outcome) => outcome,
        RunResult::Suspended(_) => panic!("a settled cell must not park again"),
    };

    let written: std::collections::HashMap<u32, Value> = outcome.signals.into_iter().collect();
    assert_eq!(
        written.get(&2),
        Some(&Value::Int(42)),
        "the awaited value must reach the handler's tail through r0"
    );
}

#[test]
fn the_wire_value_is_what_the_handler_observes() {
    // A value that is not a small int, to catch a codec that only moves i64s.
    let mut signals = InMemorySignals::default();
    let (state, cell) = suspend_once(&mut signals);

    let sent = Value::Str(4242);
    let delivered = ResumeFrame::from_bytes(&ResumeFrame::ready(1, cell, sent.clone()).to_bytes())
        .expect("round-trips");
    assert_eq!(delivered.value, sent, "the payload must survive the wire");

    signals.resolve_cell(cell, delivered.value.clone());
    let outcome = match resume(state, &mut signals, delivered.value).expect("resumes") {
        RunResult::Halt(outcome) => outcome,
        RunResult::Suspended(_) => panic!("must not park again"),
    };
    let written: std::collections::HashMap<u32, Value> = outcome.signals.into_iter().collect();
    assert_eq!(written.get(&2), Some(&sent));
}

#[test]
fn signal_writes_made_before_the_suspend_survive_the_round_trip() {
    // A handler that writes s5 BEFORE awaiting, so the suspend snapshot has real
    // work to preserve: WRITE_SIGNAL s5, r0 · CALL_CAP · AWAIT · WRITE_SIGNAL s2 · HALT.
    // Losing this write would mean a handler silently forgets work across an await.
    let mut program = vec![0x11u8, 5, 0, 0, 0, 0];
    program.extend_from_slice(&call_cap(2, 2, 99, 0));
    program.extend_from_slice(&[0xE0u8, 0, 2]);
    program.extend_from_slice(&[0x11u8, 2, 0, 0, 0, 0]);
    program.push(0x00u8);

    let mut signals = InMemorySignals::default();
    let state = match run_resumable(&program, &mut signals, Value::Int(7)).expect("runs") {
        RunResult::Suspended(state) => state,
        RunResult::Halt(_) => panic!("the async capability must park"),
    };
    assert!(
        state.signals.iter().any(|(id, _)| *id == 5),
        "the pre-await write must be captured in the suspend snapshot"
    );
    let cell = match &state.registers[usize::from(state.future_reg)] {
        Value::Int(id) => u32::try_from(*id).expect("a cell id is a u32"),
        other => panic!("future register must hold the cell id, got {other:?}"),
    };

    signals.resolve_cell(cell, Value::Int(1));
    let outcome = match resume(state, &mut signals, Value::Int(1)).expect("resumes") {
        RunResult::Halt(outcome) => outcome,
        RunResult::Suspended(_) => panic!("must not park again"),
    };
    let written: std::collections::HashMap<u32, Value> = outcome.signals.into_iter().collect();
    assert_eq!(
        written.get(&5),
        Some(&Value::Int(7)),
        "the pre-await write is replayed, not dropped"
    );
    assert_eq!(
        written.get(&2),
        Some(&Value::Int(1)),
        "and the resumed tail still runs"
    );
}

#[test]
fn an_error_resume_is_distinguishable_from_a_value_resume() {
    // A faulting capability must not be delivered as a successful result
    // (ADR-0044): the handler has to be able to take its error path.
    let mut signals = InMemorySignals::default();
    let (_, cell) = suspend_once(&mut signals);

    let frame = ResumeFrame::error(1, cell, Value::Str(9));
    let decoded = ResumeFrame::from_bytes(&frame.to_bytes()).expect("round-trips");
    assert!(decoded.is_error, "the error flag must survive the wire");
    assert_eq!(decoded.cell, cell);
}

//! The reference VM interpreter.
//!
//! This is the behavioral oracle for the Flux instruction set (Appendix E). The
//! Swift (FLUX-006) and Kotlin (FLUX-007) runtimes must match its observable
//! outputs on every vector under `/tests/isa-vectors/`. The semantics encoded
//! here incorporate the resolutions in ADR-0021 (HALT is free), ADR-0022
//! (lengths from the width table), ADR-0023 (integer DIV/MOD by zero raises
//! `DivByZero`; float DIV by zero is IEEE `±inf`), and ADR-0024 (`GET_FIELD` on
//! `Null` raises `NullDereference`, other non-records raise `TypeMismatch`).

use flux_syntax::opcode::Opcode;
use flux_syntax::{PropIdx, SignalId, Value};

use crate::decode::{Instruction, decode_program};
use crate::error::{VmError, VmErrorKind};

/// Result of running a handler to completion.
#[derive(Clone, Debug, PartialEq)]
pub struct VmOutcome {
    /// Final values of all signal cells that were written.
    pub signals: Vec<(SignalId, Value)>,
    /// Final values of the 16 registers (r0 = entry payload, r15 = remaining gas).
    pub registers: [Value; 16],
    /// Number of non-`HALT` instructions executed (ADR-0021).
    pub gas_used: u32,
}

/// The captured continuation of a suspended handler (ADR-0044, MLP v2 async).
///
/// The reference VM is a flat register machine with no call stack, so a suspend is
/// exactly its live interpreter state: the next instruction offset, the register
/// file, the remaining gas, and the snapshot of signal cells that had been written
/// before the `AWAIT`. [`resume`] re-enters the interpreter at `resume_ip` with the
/// delivered value placed in register `r0`.
#[derive(Clone, Debug, PartialEq)]
pub struct SuspendState {
    /// The original bytecode program, re-decoded on resume so the tail can be
    /// executed from `resume_ip` without the caller retaining the bytes.
    pub program: Vec<u8>,
    /// Byte offset of the instruction to execute on resume (the byte after `AWAIT`).
    pub resume_ip: u32,
    /// Register file at the point of suspension.
    pub registers: [Value; 16],
    /// Remaining gas at the point of suspension; continues decrementing on resume.
    pub gas_remaining: u32,
    /// Signal cells written before the suspend, replayed into the graph on resume.
    pub signals: Vec<(SignalId, Value)>,
    /// The register holding the awaited future handle at the point of suspension.
    /// The executor reads `registers[future_reg]` to obtain the future it must
    /// resolve, then resumes with the resolved value in `r0`.
    pub future_reg: u8,
}

/// The outcome of a (possibly resumable) handler run.
///
/// `Halt` is the v1 terminal result and is byte-for-byte what [`run`] returns.
/// `Suspended` is new for v2: the handler hit `AWAIT` and must be continued via
/// [`resume`]. This enum is additive — existing v1 callers that use [`run`] never
/// observe `Suspended`.
#[derive(Clone, Debug, PartialEq)]
pub enum RunResult {
    /// The handler reached `HALT` and produced a final outcome.
    Halt(VmOutcome),
    /// The handler suspended on `AWAIT`; continue it with [`resume`].
    Suspended(SuspendState),
}

/// Reactive state of a signal cell (ADR-0045, MLP v2 async capability bridge).
///
/// A capability call creates a result cell. A *synchronous* method writes
/// `Ready(value)` before returning; an *asynchronous* method leaves the cell
/// `Pending` and the host resolves it later (writing `Ready` or `Error`). `AWAIT`
/// parks while the cell is `Pending` and consumes the value once `Ready`.
#[derive(Clone, Debug, PartialEq)]
pub enum CellState {
    /// The cell has settled with `value`.
    Ready(Value),
    /// The cell is waiting on an in-flight async capability.
    Pending,
    /// The capability faulted; `value` is the error payload.
    Error(Value),
}

/// The signal graph a handler reads from and writes to.
pub trait SignalStore {
    /// Returns the current value of `id`, or `None` if unbound.
    fn read(&self, id: SignalId) -> Option<Value>;
    /// Writes `value` into `id`, resolving any pending/error cell back to `Ready`.
    fn write(&mut self, id: SignalId, value: Value);
    /// Returns every written signal as a sorted `(id, value)` list.
    ///
    /// Used by [`run`] to populate [`VmOutcome::signals`]; the oracle needs a
    /// total snapshot, not a diff, so the production runtimes can compare
    /// final state against the golden vectors.
    fn snapshot(&self) -> Vec<(SignalId, Value)>;
    /// Allocates a fresh, unbound signal id for a new capability result cell.
    fn allocate_cell(&mut self) -> SignalId;
    /// Returns the reactive [`CellState`] of `id`, defaulting to `Ready(Null)`
    /// for unbound cells (an `AWAIT` only parks on `Pending`).
    fn cell_state(&self, id: SignalId) -> CellState;
    /// Marks `id` as `Pending` (an async capability has started).
    fn mark_pending(&mut self, id: SignalId);
    /// Resolves `id` with `value`, flipping it back to `Ready(value)`.
    fn resolve_cell(&mut self, id: SignalId, value: Value);
}

/// In-memory [`SignalStore`] used by tests and the dev server.
#[derive(Clone, Debug)]
pub struct InMemorySignals {
    values: std::collections::HashMap<SignalId, Value>,
    states: std::collections::HashMap<SignalId, CellState>,
    next_cell: SignalId,
}

impl Default for InMemorySignals {
    fn default() -> Self {
        Self {
            values: std::collections::HashMap::new(),
            states: std::collections::HashMap::new(),
            // Fresh result cells start well above the low, fixed ids that golden
            // vectors and handlers use (e.g. 99), so `allocate_cell` never collides.
            next_cell: 1_000_000,
        }
    }
}

impl SignalStore for InMemorySignals {
    fn read(&self, id: SignalId) -> Option<Value> {
        self.values.get(&id).cloned()
    }
    fn write(&mut self, id: SignalId, value: Value) {
        self.values.insert(id, value.clone());
        self.states.insert(id, CellState::Ready(value));
    }
    fn snapshot(&self) -> Vec<(SignalId, Value)> {
        let mut out: Vec<(SignalId, Value)> =
            self.values.iter().map(|(k, v)| (*k, v.clone())).collect();
        out.sort_by_key(|(k, _)| *k);
        out
    }
    fn allocate_cell(&mut self) -> SignalId {
        // Skip ids already used by the program (golden vectors use low, fixed ids
        // like 99) by drawing from a high ceiling reserved for runtime allocation.
        self.next_cell += 1;
        self.next_cell
    }
    fn cell_state(&self, id: SignalId) -> CellState {
        if let Some(state) = self.states.get(&id) {
            return state.clone();
        }
        match self.values.get(&id) {
            Some(v) => CellState::Ready(v.clone()),
            None => CellState::Ready(Value::Null),
        }
    }
    fn mark_pending(&mut self, id: SignalId) {
        self.states.insert(id, CellState::Pending);
    }
    fn resolve_cell(&mut self, id: SignalId, value: Value) {
        self.values.insert(id, value.clone());
        self.states.insert(id, CellState::Ready(value));
    }
}

impl InMemorySignals {
    /// Builds a store from an iterator of `(id, value)` pairs.
    #[must_use]
    pub fn from_signals(signals: impl IntoIterator<Item = (SignalId, Value)>) -> Self {
        let values: std::collections::HashMap<SignalId, Value> = signals.into_iter().collect();
        let states = values
            .iter()
            .map(|(k, v)| (*k, CellState::Ready(v.clone())))
            .collect();
        Self {
            values,
            states,
            next_cell: 1_000_000,
        }
    }
}

/// A capability implementation invoked by `CALL_CAP` (ADR-0045, unified sync/async bridge).
///
/// The impl creates a result cell in `signals` and returns its [`SignalId`]:
/// - a **synchronous** method writes `Ready(value)` into the cell before returning;
/// - an **asynchronous** method leaves the cell `Pending`; the host resolves it later
///   (writing `Ready`/`Error`) which resumes any awaiting handler.
///
/// One signature serves both shapes; the VM never branches on sync-vs-async.
pub type CapabilityImpl =
    fn(cap_id: u32, method_id: u16, args: &Value, signals: &mut dyn SignalStore) -> SignalId;

/// A data-driven registry mapping `(capId, methodId)` pairs to their [`CapabilityImpl`].
///
/// Mirrors the host `CapabilityRegistry` tables (Registry.swift / CapabilityRegistry.kt);
/// the oracle uses [`CapabilityRegistry::with_parity_stubs`] so existing golden vectors (e.g.
/// `call_cap_basic`, cap 1/1 → signal 99) stay green under the v2 signal-id contract.
#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    entries: Vec<(u32, u16, CapabilityImpl)>,
}

impl CapabilityRegistry {
    /// Looks up the implementation for `(cap_id, method_id)`, or `None` if unregistered
    /// (the oracle then faults with `TypeMismatch`, matching the "capability must exist" contract).
    #[must_use]
    pub fn lookup(&self, cap_id: u32, method_id: u16) -> Option<CapabilityImpl> {
        self.entries
            .iter()
            .rev()
            .find(|(c, m, _)| *c == cap_id && *m == method_id)
            .map(|(_, _, f)| *f)
    }

    /// The default oracle registry: the v1 parity stub `Camera.take` (cap 1, method 1)
    /// echoes `arg[0]` into signal 99 (already `Ready`) and returns 99.
    #[must_use]
    pub fn with_parity_stubs() -> Self {
        Self {
            entries: vec![
                (1, 1, parity_echo_99),
                // Reference async capability: returns a Pending cell (cap 2, method 99),
                // exercised by the suspend/resume bridge test.
                (2, 99, async_deferred),
            ],
        }
    }
}

/// v1 parity stub: `Camera.take(arg)` writes `arg[0]` into signal 99 and returns 99.
///
/// Golden `call_cap_basic` (capId=1, methodId=1) depends on this exact behavior; the
/// v2 change is that the result register receives the cell id (99) rather than the raw
/// arg, keeping the echo into signal 99 intact.
fn parity_echo_99(
    _cap_id: u32,
    _method_id: u16,
    args: &Value,
    signals: &mut dyn SignalStore,
) -> SignalId {
    let arg = match args {
        Value::Record(fields) if !fields.is_empty() => fields[0].1.clone(),
        _ => Value::Null,
    };
    signals.write(99, arg);
    // `write` resolves the cell to Ready; the returned id is the result cell.
    99
}

/// Reference async capability (cap 2, method 99): allocates a fresh result cell, marks it
/// `Pending`, and returns its id immediately. The host resolves it later via
/// `SignalStore::resolve_cell`, which resumes any awaiting handler (ADR-0045 §1). This is
/// the oracle's reference async method used to exercise the suspend/resume bridge.
fn async_deferred(
    _cap_id: u32,
    _method_id: u16,
    _args: &Value,
    signals: &mut dyn SignalStore,
) -> SignalId {
    let id = signals.allocate_cell();
    signals.mark_pending(id);
    id
}

const ENTRY_GAS: u32 = 100_000;

/// Returns the byte offset of the instruction that follows `instr` in the program.
#[must_use]
fn next_offset(instr: &Instruction) -> u32 {
    instr.offset + u32::from(instr.opcode.instruction_len())
}

/// Runs `bytecode` with resumable semantics, returning either a final [`VmOutcome`]
/// or a [`RunResult::Suspended`] continuation at the first `AWAIT` (ADR-0044).
///
/// This is the v2 entry point for async-capable handlers. The v1 [`run`] is a thin
/// wrapper that asserts the handler never suspends (it always reaches `HALT`), so
/// existing callers and ISA golden vectors are unaffected.
///
/// # Errors
///
/// Returns a [`VmError`] when the handler faults before halting or suspending
/// (gas exhaustion, bad dispatch, type error, out-of-bounds access, null
/// dereference, or division by zero).
pub fn run_resumable(
    bytecode: &[u8],
    signals: &mut impl SignalStore,
    payload: Value,
) -> Result<RunResult, VmError> {
    let program = decode_program(bytecode)?;
    let offsets: Vec<u32> = program.iter().map(|i| i.offset).collect();
    let mut regs = std::array::from_fn(|_| Value::Null);
    regs[0] = payload;
    regs[15] = Value::Int(i64::from(ENTRY_GAS));
    let mut gas: u32 = ENTRY_GAS;

    match exec_tail(&program, &offsets, 0, &mut regs, &mut gas, signals)? {
        ControlFlow::Halt => Ok(finish(regs, gas, signals)),
        ControlFlow::Suspend {
            resume_ip,
            future_reg,
        } => {
            let written = snapshot_sorted(signals);
            Ok(RunResult::Suspended(SuspendState {
                program: bytecode.to_vec(),
                resume_ip,
                registers: regs,
                gas_remaining: gas,
                signals: written,
                future_reg,
            }))
        }
    }
}

/// Continues a suspended handler (ADR-0044), delivering `value` as the awaited result.
///
/// Re-enters the interpreter at [`SuspendState::resume_ip`] with `value` in `r0` and
/// the captured registers/gas restored. The signal writes captured at suspend time are
/// folded back into `signals` before resuming so subsequent reads observe them.
///
/// # Errors
///
/// Returns a [`VmError`] if the resumed handler faults (same fault classes as
/// [`run_resumable`]).
pub fn resume(
    state: SuspendState,
    signals: &mut impl SignalStore,
    value: Value,
) -> Result<RunResult, VmError> {
    // Replay the signal writes captured at suspend so reads during the resumed tail
    // see the pre-suspend state.
    for (id, v) in &state.signals {
        signals.write(*id, v.clone());
    }
    let program = decode_program(&state.program)?;
    let offsets: Vec<u32> = program.iter().map(|i| i.offset).collect();
    let mut regs = state.registers;
    regs[0] = value; // The awaited value lands in r0.
    let mut gas = state.gas_remaining;

    match exec_tail(
        &program,
        &offsets,
        state.resume_ip,
        &mut regs,
        &mut gas,
        signals,
    )? {
        ControlFlow::Halt => Ok(finish(regs, gas, signals)),
        ControlFlow::Suspend {
            resume_ip,
            future_reg,
        } => {
            let written = snapshot_sorted(signals);
            Ok(RunResult::Suspended(SuspendState {
                program: state.program,
                resume_ip,
                registers: regs,
                gas_remaining: gas,
                signals: written,
                future_reg,
            }))
        }
    }
}

/// How the shared tail interpreter terminated.
enum ControlFlow {
    /// Reached `HALT`; the caller assembles the final [`VmOutcome`].
    Halt,
    /// Hit `AWAIT`; `resume_ip` is the byte offset of the next instruction and
    /// `future_reg` is the register holding the awaited future handle.
    Suspend { resume_ip: u32, future_reg: u8 },
}

/// Sorts a signal snapshot for deterministic suspension capture.
fn snapshot_sorted(signals: &mut impl SignalStore) -> Vec<(SignalId, Value)> {
    let mut out = signals.snapshot();
    out.sort_by_key(|(k, _)| *k);
    out
}

/// Executes instructions starting at `start_offset` until `HALT` or `AWAIT`.
///
/// Shared by [`run_resumable`] (entry from offset 0) and [`resume`] (entry from the
/// captured `resume_ip`). Every opcode except `AWAIT` is evaluated here; `AWAIT`
/// returns [`ControlFlow::Suspend`] with the offset of the following instruction.
fn exec_tail(
    program: &[Instruction],
    offsets: &[u32],
    start_offset: u32,
    regs: &mut [Value; 16],
    gas: &mut u32,
    signals: &mut impl SignalStore,
) -> Result<ControlFlow, VmError> {
    let start_index = offsets
        .iter()
        .position(|&o| o == start_offset)
        .unwrap_or(program.len());
    let mut ip_index = start_index;

    while ip_index < program.len() {
        let instr: &Instruction = &program[ip_index];
        let op = instr.opcode;
        if op == Opcode::Halt {
            return Ok(ControlFlow::Halt);
        }
        if *gas == 0 {
            return Err(VmError::at(VmErrorKind::GasExhausted, instr.offset));
        }
        *gas -= 1;
        // Mirror the live gas budget into r15 (Appendix E §E.3; ADR-0021).
        regs[15] = Value::Int(i64::from(*gas));
        let next_index = ip_index + 1;

        macro_rules! reg {
            ($r:expr) => {
                regs[usize::from($r)].clone()
            };
        }

        match op {
            Opcode::Await => {
                // `future_reg` holds the register containing the result-cell signal id
                // returned by CALL_CAP (ADR-0045). Park only while the cell is `Pending`;
                // a `Ready` cell continues with its value in r0 (one re-entry, no real park),
                // and an `Error` cell faults the handler rather than resuming.
                let future_reg = instr.u8(1);
                let cell_id = match regs[usize::from(future_reg)] {
                    Value::Int(n) if n >= 0 => n as SignalId,
                    _ => return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset)),
                };
                let st = signals.cell_state(cell_id);
                match st {
                    CellState::Ready(value) => {
                        regs[0] = value;
                    }
                    CellState::Pending => {
                        return Ok(ControlFlow::Suspend {
                            resume_ip: next_offset(instr),
                            future_reg,
                        });
                    }
                    CellState::Error(_) => {
                        return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset));
                    }
                }
            }
            Opcode::Nop => {}
            Opcode::ReadSignal => {
                let dst = instr.u8(0);
                let id = instr.u32(1);
                regs[usize::from(dst)] = signals.read(id).unwrap_or(Value::Null);
            }
            Opcode::WriteSignal => {
                let id = instr.u32(0);
                let src = instr.u8(4);
                signals.write(id, reg!(src));
            }
            Opcode::EqI64 | Opcode::LtI64 | Opcode::GtI64 | Opcode::LteI64 | Opcode::GteI64 => {
                let dst = instr.u8(0);
                let a = reg!(instr.u8(1));
                let b = reg!(instr.u8(2));
                let (x, y) = expect_ints(a, b, instr.offset)?;
                let r = match op {
                    Opcode::EqI64 => x == y,
                    Opcode::LtI64 => x < y,
                    Opcode::GtI64 => x > y,
                    Opcode::LteI64 => x <= y,
                    Opcode::GteI64 => x >= y,
                    _ => unreachable!(),
                };
                regs[usize::from(dst)] = Value::Bool(r);
            }
            Opcode::AddI64 | Opcode::SubI64 | Opcode::MulI64 | Opcode::DivI64 | Opcode::ModI64 => {
                let dst = instr.u8(0);
                let a = reg!(instr.u8(1));
                let b = reg!(instr.u8(2));
                let (x, y) = expect_ints(a, b, instr.offset)?;
                let r = match op {
                    Opcode::AddI64 => x.wrapping_add(y),
                    Opcode::SubI64 => x.wrapping_sub(y),
                    Opcode::MulI64 => x.wrapping_mul(y),
                    Opcode::DivI64 => {
                        if y == 0 {
                            return Err(VmError::at(VmErrorKind::DivByZero, instr.offset));
                        }
                        x.wrapping_div(y)
                    }
                    Opcode::ModI64 => {
                        if y == 0 {
                            return Err(VmError::at(VmErrorKind::DivByZero, instr.offset));
                        }
                        x.wrapping_rem(y)
                    }
                    _ => unreachable!(),
                };
                regs[usize::from(dst)] = Value::Int(r);
            }
            Opcode::NegI64 => {
                let dst = instr.u8(0);
                let v = expect_int(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(dst)] = Value::Int(-v);
            }
            Opcode::EqF64 | Opcode::LtF64 | Opcode::GtF64 => {
                let dst = instr.u8(0);
                let a = reg!(instr.u8(1));
                let b = reg!(instr.u8(2));
                let (x, y) = expect_floats(a, b, instr.offset)?;
                let r = match op {
                    Opcode::EqF64 => (x == y) || (x.is_nan() && y.is_nan()),
                    Opcode::LtF64 => x < y,
                    Opcode::GtF64 => x > y,
                    _ => unreachable!(),
                };
                regs[usize::from(dst)] = Value::Bool(r);
            }
            Opcode::AddF64 | Opcode::SubF64 | Opcode::MulF64 | Opcode::DivF64 => {
                let dst = instr.u8(0);
                let a = reg!(instr.u8(1));
                let b = reg!(instr.u8(2));
                let (x, y) = expect_floats(a, b, instr.offset)?;
                let r = match op {
                    Opcode::AddF64 => x + y,
                    Opcode::SubF64 => x - y,
                    Opcode::MulF64 => x * y,
                    Opcode::DivF64 => fdiv(x, y),
                    _ => unreachable!(),
                };
                regs[usize::from(dst)] = Value::Float(r);
            }
            Opcode::NegF64 => {
                let dst = instr.u8(0);
                let v = expect_float(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(dst)] = Value::Float(-v);
            }
            Opcode::I64ToF64 => {
                let dst = instr.u8(0);
                let v = expect_int(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(dst)] = Value::Float(v as f64);
            }
            Opcode::F64ToI64 => {
                let dst = instr.u8(0);
                let v = expect_float(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(dst)] = Value::Int(v as i64);
            }
            Opcode::AndBool => {
                let (x, y) = expect_bools(reg!(instr.u8(1)), reg!(instr.u8(2)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Bool(x && y);
            }
            Opcode::OrBool => {
                let (x, y) = expect_bools(reg!(instr.u8(1)), reg!(instr.u8(2)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Bool(x || y);
            }
            Opcode::NotBool => {
                let v = expect_bool(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Bool(!v);
            }
            Opcode::StrIntern => {
                regs[usize::from(instr.u8(0))] = Value::Str(instr.u32(1));
            }
            Opcode::StrEq => {
                let (x, y) = expect_strs(reg!(instr.u8(1)), reg!(instr.u8(2)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Bool(x == y);
            }
            Opcode::StrLen => {
                let id = expect_str(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Int(i64::from(id.ilog10() + 1));
            }
            Opcode::StrConcat => {
                let (x, y) = expect_strs(reg!(instr.u8(1)), reg!(instr.u8(2)), instr.offset)?;
                regs[usize::from(instr.u8(0))] =
                    Value::Str(x.wrapping_mul(10_000_000).wrapping_add(y));
            }
            Opcode::ToString => {
                let rendered = render_value(reg!(instr.u8(1)));
                regs[usize::from(instr.u8(0))] = Value::Str(synthetic_str_id(&rendered));
            }
            Opcode::Jump => {
                ip_index = jump_target(instr, next_index, offsets, instr.i32(0))?;
                continue;
            }
            Opcode::CondJump | Opcode::CondJumpNot => {
                let taken = truthy(&regs[usize::from(instr.u8(0))]);
                let want = op == Opcode::CondJump;
                if taken == want {
                    ip_index = jump_target(instr, next_index, offsets, instr.i32(1))?;
                    continue;
                }
            }
            Opcode::AllocRecord => {
                let dst = instr.u8(0);
                let count = usize::from(instr.u16(1));
                let mut fields = Vec::with_capacity(count);
                for i in 0..count {
                    fields.push((PropIdx::from(i as u16), Value::Null));
                }
                regs[usize::from(dst)] = Value::Record(fields);
            }
            Opcode::GetField => {
                let dst = instr.u8(0);
                let idx = instr.u16(1);
                let obj = reg!(instr.u8(3));
                let field = get_field(obj, idx, instr.offset)?;
                regs[usize::from(dst)] = field;
            }
            Opcode::SetField => {
                let obj = instr.u8(0);
                let idx = instr.u16(1);
                let val = reg!(instr.u8(3));
                set_field(&mut regs[usize::from(obj)], idx, val, instr.offset)?;
            }
            Opcode::RecordEq => {
                let (x, y) = expect_records(reg!(instr.u8(1)), reg!(instr.u8(2)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Bool(x == y);
            }
            Opcode::AllocList => {
                let cap = instr.u16(1);
                regs[usize::from(instr.u8(0))] = Value::List(Vec::with_capacity(usize::from(cap)));
            }
            Opcode::ListPush => {
                let list = instr.u8(0);
                let val = reg!(instr.u8(1));
                match &mut regs[usize::from(list)] {
                    Value::List(items) => items.push(val),
                    _ => return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset)),
                }
            }
            Opcode::ListGet => {
                let dst = instr.u8(0);
                let (items, i) = expect_list_index(reg!(instr.u8(1)), instr.u8(2), instr.offset)?;
                regs[usize::from(dst)] = items[i].clone();
            }
            Opcode::ListLen => {
                let items = expect_list(reg!(instr.u8(1)), instr.offset)?;
                regs[usize::from(instr.u8(0))] = Value::Int(items.len() as i64);
            }
            Opcode::ListConcat => {
                let (a, b) = expect_lists(reg!(instr.u8(1)), reg!(instr.u8(2)), instr.offset)?;
                let mut items = a.clone();
                items.extend(b.iter().cloned());
                regs[usize::from(instr.u8(0))] = Value::List(items);
            }
            Opcode::CallCap => {
                let result_reg = instr.u8(0);
                let cap_id = instr.u32(1);
                let method_id = instr.u16(5);
                let args_reg = instr.u8(7);
                // Unified sync/async capability bridge (ADR-0045): the impl creates a
                // result cell and returns its signal id; `result_reg` receives that id.
                // An unregistered `(capId, methodId)` is a type error (the VM cannot
                // invent a capability), matching the host contract.
                let args = regs[usize::from(args_reg)].clone();
                let registry = CapabilityRegistry::with_parity_stubs();
                match registry.lookup(cap_id, method_id) {
                    Some(impl_) => {
                        let id = impl_(cap_id, method_id, &args, signals);
                        regs[usize::from(result_reg)] = Value::Int(i64::from(id));
                    }
                    None => return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset)),
                }
            }
            Opcode::MatchTag => {
                let val = reg!(instr.u8(0));
                let tag = instr.u32(1);
                let mut matched = false;
                if let Value::Record(fields) = &val {
                    if let Some((_, Value::Int(t))) = fields.first() {
                        matched = *t == i64::from(tag);
                    }
                }
                if matched {
                    ip_index = jump_target(instr, next_index, offsets, instr.i32(5))?;
                    continue;
                }
            }
            Opcode::ExtractField => {
                let dst = instr.u8(0);
                let idx = instr.u16(1);
                let val = reg!(instr.u8(3));
                let field = get_field(val, idx, instr.offset)?;
                regs[usize::from(dst)] = field;
            }
            Opcode::LoadIntConst => {
                regs[usize::from(instr.u8(0))] = Value::Int(instr.i64(1));
            }
            Opcode::LoadFloatConst => {
                regs[usize::from(instr.u8(0))] = Value::Float(instr.f64(1));
            }
            Opcode::LoadBoolConst => {
                regs[usize::from(instr.u8(0))] = Value::Bool(instr.u8(1) != 0);
            }
            Opcode::LoadStrConst => {
                regs[usize::from(instr.u8(0))] = Value::Str(instr.u32(1));
            }
            Opcode::LoadNull => {
                regs[usize::from(instr.u8(0))] = Value::Null;
            }
            Opcode::Mov => {
                regs[usize::from(instr.u8(0))] = reg!(instr.u8(1));
            }
            Opcode::GasCheck => {
                let budget = instr.u32(0);
                if *gas < budget {
                    return Err(VmError::at(VmErrorKind::GasExhausted, instr.offset));
                }
            }
            Opcode::Halt => return Ok(ControlFlow::Halt),
            // `Opcode` is `#[non_exhaustive]`; the decoder only yields known variants.
            _ => return Err(VmError::at(VmErrorKind::InvalidDispatch, instr.offset)),
        }
        ip_index = next_index;
    }
    Ok(ControlFlow::Halt)
}

/// Assembles the terminal [`VmOutcome`] from the live interpreter state.
fn finish(regs: [Value; 16], gas: u32, signals: &mut impl SignalStore) -> RunResult {
    let out_signals = signals.snapshot();
    RunResult::Halt(VmOutcome {
        signals: out_signals,
        registers: regs,
        gas_used: ENTRY_GAS - gas,
    })
}

/// Runs `bytecode` to completion against `signals`, with `payload` in `r0`.
///
/// This is the v1 entry point. v1 handlers never emit `AWAIT` (that opcode is an
/// MLP v2 addition, ADR-0044), so this always reaches `HALT` and returns a
/// [`VmOutcome`]. It delegates to the shared `exec_tail` interpreter used by the
/// resumable [`run_resumable`] / [`resume`] path, so the two execution models stay
/// in lockstep and cannot drift.
///
/// # Errors
///
/// Returns a [`VmError`] when the handler faults (gas exhaustion, bad dispatch,
/// type error, out-of-bounds access, null dereference, or division by zero).
pub fn run(
    bytecode: &[u8],
    signals: &mut impl SignalStore,
    payload: Value,
) -> Result<VmOutcome, VmError> {
    let program = decode_program(bytecode)?;
    // v1 bytecode never emits `AWAIT` (an MLP v2 opcode, ADR-0044). If one is present the
    // program is malformed for the v1 entry point: reject it rather than silently running the
    // resumable interpreter, which would otherwise continue past a `Ready` cell. This keeps
    // the v1 contract (no suspension) intact while `run_resumable` handles async handlers.
    if program
        .iter()
        .any(|instr| matches!(instr.opcode, Opcode::Await))
    {
        return Err(VmError::at(VmErrorKind::InvalidDispatch, 0));
    }
    let offsets: Vec<u32> = program.iter().map(|i| i.offset).collect();
    let mut regs = std::array::from_fn(|_| Value::Null);
    regs[0] = payload;
    regs[15] = Value::Int(i64::from(ENTRY_GAS));
    let mut gas: u32 = ENTRY_GAS;

    match exec_tail(&program, &offsets, 0, &mut regs, &mut gas, signals)? {
        ControlFlow::Halt => {
            let out_signals = signals.snapshot();
            Ok(VmOutcome {
                signals: out_signals,
                registers: regs,
                gas_used: ENTRY_GAS - gas,
            })
        }
        // v1 bytecode contains no `AWAIT`, so this arm is unreachable for v1 callers.
        ControlFlow::Suspend { .. } => Err(VmError::at(VmErrorKind::InvalidDispatch, 0)),
    }
}

/// IEEE-754 division: `x/0.0` is `±inf` (ADR-0023), never an error.
fn fdiv(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        if x.is_nan() {
            return f64::NAN;
        }
        return if x >= 0.0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
    }
    x / y
}

/// Renders `value` as the text `TO_STRING` (0xD0) produces. The production
/// runtimes render the same shapes; only `Value::Str` differs, because the
/// oracle has no live string table and therefore cannot resolve an id back to
/// its text (it renders the id, which the vectors pin).
fn render_value(value: Value) -> String {
    match value {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format_float(f),
        Value::Bool(b) => b.to_string(),
        Value::Str(id) => id.to_string(),
        Value::Null => "null".to_owned(),
        Value::HandlerRef(id) => format!("handler({id})"),
        Value::List(items) => {
            let rendered: Vec<String> = items.into_iter().map(render_value).collect();
            format!("[{}]", rendered.join(", "))
        }
        Value::Record(fields) => {
            let rendered: Vec<String> = fields
                .into_iter()
                .map(|(idx, v)| format!("{idx}: {}", render_value(v)))
                .collect();
            format!("{{{}}}", rendered.join(", "))
        }
        // `Value` is `#[non_exhaustive]`: a variant added later has no agreed
        // rendering across the three runtimes yet, so it renders as `null`
        // rather than silently inventing text the hosts would not reproduce.
        _ => "null".to_owned(),
    }
}

/// Formats a float the way every runtime must: an integral value keeps a single
/// fractional digit (`1.0`), so Rust, Swift and Kotlin agree byte for byte.
fn format_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

/// Derives the [`flux_syntax::StringId`] the oracle assigns to text it had to
/// synthesise. The oracle owns no string table, so it interns into the reserved
/// high half (mirroring the hosts' reverse-intern range) via FNV-1a. This keeps
/// `TO_STRING` deterministic and self-consistent within one program run.
fn synthetic_str_id(text: &str) -> flux_syntax::StringId {
    let mut hash: u32 = 0x811c_9dc5;
    for &byte in text.as_bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    0x8000_0000 | (hash & 0x7FFF_FFFF)
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        _ => false,
    }
}

fn expect_int(v: Value, off: u32) -> Result<i64, VmError> {
    v.as_int()
        .ok_or_else(|| VmError::at(VmErrorKind::TypeMismatch, off))
}

fn expect_float(v: Value, off: u32) -> Result<f64, VmError> {
    v.as_float()
        .ok_or_else(|| VmError::at(VmErrorKind::TypeMismatch, off))
}

fn expect_bool(v: Value, off: u32) -> Result<bool, VmError> {
    v.as_bool()
        .ok_or_else(|| VmError::at(VmErrorKind::TypeMismatch, off))
}

fn expect_ints(a: Value, b: Value, off: u32) -> Result<(i64, i64), VmError> {
    Ok((expect_int(a, off)?, expect_int(b, off)?))
}

fn expect_floats(a: Value, b: Value, off: u32) -> Result<(f64, f64), VmError> {
    Ok((expect_float(a, off)?, expect_float(b, off)?))
}

fn expect_bools(a: Value, b: Value, off: u32) -> Result<(bool, bool), VmError> {
    Ok((expect_bool(a, off)?, expect_bool(b, off)?))
}

fn expect_str(v: Value, off: u32) -> Result<flux_syntax::StringId, VmError> {
    v.as_str_id()
        .ok_or_else(|| VmError::at(VmErrorKind::TypeMismatch, off))
}

fn expect_strs(a: Value, b: Value, off: u32) -> Result<(u32, u32), VmError> {
    Ok((expect_str(a, off)?, expect_str(b, off)?))
}

fn expect_list(v: Value, off: u32) -> Result<Vec<Value>, VmError> {
    match v {
        Value::List(items) => Ok(items),
        _ => Err(VmError::at(VmErrorKind::TypeMismatch, off)),
    }
}

fn expect_lists(a: Value, b: Value, off: u32) -> Result<(Vec<Value>, Vec<Value>), VmError> {
    Ok((expect_list(a, off)?, expect_list(b, off)?))
}

/// The field list of a `Value::Record`: an ordered `(property, value)` list.
type RecordFields = Vec<(PropIdx, Value)>;

fn expect_records(a: Value, b: Value, off: u32) -> Result<(RecordFields, RecordFields), VmError> {
    match (a, b) {
        (Value::Record(x), Value::Record(y)) => Ok((x, y)),
        _ => Err(VmError::at(VmErrorKind::TypeMismatch, off)),
    }
}

fn expect_list_index(list: Value, idx: u8, off: u32) -> Result<(Vec<Value>, usize), VmError> {
    let items = expect_list(list, off)?;
    let i = usize::from(idx);
    if i >= items.len() {
        return Err(VmError::at(VmErrorKind::IndexOutOfBounds, off));
    }
    Ok((items, i))
}

/// Resolves a relative jump offset (relative to the *next* instruction) to a
/// program index, or `IndexOutOfBounds` if it lands outside the program.
fn jump_target(
    instr: &Instruction,
    next_index: usize,
    offsets: &[u32],
    offset: i32,
) -> Result<usize, VmError> {
    // `offsets[next_index]` is the byte offset of the instruction immediately
    // after the jumping instruction, which is the anchor the offset is measured
    // from (Appendix E §E.4).
    let base = offsets
        .get(next_index)
        .copied()
        .ok_or_else(|| VmError::at(VmErrorKind::IndexOutOfBounds, instr.offset))?;
    let target_offset = i64::from(base) + i64::from(offset);
    let target = u32::try_from(target_offset)
        .map_err(|_| VmError::at(VmErrorKind::IndexOutOfBounds, instr.offset))?;
    offsets
        .iter()
        .position(|&o| o == target)
        .ok_or_else(|| VmError::at(VmErrorKind::IndexOutOfBounds, instr.offset))
}

fn get_field(obj: Value, idx: u16, off: u32) -> Result<Value, VmError> {
    if let Value::Null = obj {
        return Err(VmError::at(VmErrorKind::NullDereference, off));
    }
    match obj {
        Value::Record(fields) => {
            let i = usize::from(idx);
            fields
                .get(i)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| VmError::at(VmErrorKind::IndexOutOfBounds, off))
        }
        _ => Err(VmError::at(VmErrorKind::TypeMismatch, off)),
    }
}

fn set_field(obj: &mut Value, idx: u16, val: Value, off: u32) -> Result<(), VmError> {
    if let Value::Null = obj {
        return Err(VmError::at(VmErrorKind::NullDereference, off));
    }
    match obj {
        Value::Record(fields) => {
            let i = usize::from(idx);
            if i >= fields.len() {
                return Err(VmError::at(VmErrorKind::IndexOutOfBounds, off));
            }
            fields[i].1 = val;
            Ok(())
        }
        _ => Err(VmError::at(VmErrorKind::TypeMismatch, off)),
    }
}

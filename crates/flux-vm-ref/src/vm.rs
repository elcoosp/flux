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

/// The signal graph a handler reads from and writes to.
pub trait SignalStore {
    /// Returns the current value of `id`, or `None` if unbound.
    fn read(&self, id: SignalId) -> Option<Value>;
    /// Writes `value` into `id`.
    fn write(&mut self, id: SignalId, value: Value);
    /// Returns every written signal as a sorted `(id, value)` list.
    ///
    /// Used by [`run`] to populate [`VmOutcome::signals`]; the oracle needs a
    /// total snapshot, not a diff, so the production runtimes can compare
    /// final state against the golden vectors.
    fn snapshot(&self) -> Vec<(SignalId, Value)>;
}

/// In-memory [`SignalStore`] used by tests and the dev server.
#[derive(Clone, Debug, Default)]
pub struct InMemorySignals(std::collections::HashMap<SignalId, Value>);

impl SignalStore for InMemorySignals {
    fn read(&self, id: SignalId) -> Option<Value> {
        self.0.get(&id).cloned()
    }
    fn write(&mut self, id: SignalId, value: Value) {
        self.0.insert(id, value);
    }
    fn snapshot(&self) -> Vec<(SignalId, Value)> {
        let mut out: Vec<(SignalId, Value)> = self.0.iter().map(|(k, v)| (*k, v.clone())).collect();
        out.sort_by_key(|(k, _)| *k);
        out
    }
}

impl InMemorySignals {
    /// Builds a store from an iterator of `(id, value)` pairs.
    #[must_use]
    pub fn from_signals(signals: impl IntoIterator<Item = (SignalId, Value)>) -> Self {
        Self(signals.into_iter().collect())
    }
}

const ENTRY_GAS: u32 = 100_000;

/// Runs `bytecode` to completion against `signals`, with `payload` in `r0`.
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
    let offsets: Vec<u32> = program.iter().map(|i| i.offset).collect();
    let mut regs = std::array::from_fn(|_| Value::Null);
    regs[0] = payload;
    regs[15] = Value::Int(i64::from(ENTRY_GAS));
    let mut gas: u32 = ENTRY_GAS;
    let mut ip_index = 0usize;

    while ip_index < program.len() {
        let instr: &Instruction = &program[ip_index];
        let op = instr.opcode;
        if op == Opcode::Halt {
            break;
        }
        if gas == 0 {
            return Err(VmError::at(VmErrorKind::GasExhausted, instr.offset));
        }
        gas -= 1;
        // Mirror the live gas budget into r15 (Appendix E §E.3; ADR-0021 says the
        // budget register decrements as instructions run).
        regs[15] = Value::Int(i64::from(gas));
        let next_index = ip_index + 1;

        macro_rules! reg {
            ($r:expr) => {
                regs[usize::from($r)].clone()
            };
        }

        match op {
            Opcode::Nop => {}
            Opcode::ReadSignal => {
                let dst = instr.u8(0);
                let id = instr.u32(1);
                if let Some(v) = signals.read(id) {
                    regs[usize::from(dst)] = v;
                } else {
                    regs[usize::from(dst)] = Value::Null;
                }
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
                // Length is the id's byte width as a proxy (no live table in the oracle).
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
                ip_index = jump_target(instr, next_index, &offsets, instr.i32(0))?;
                continue;
            }
            Opcode::CondJump | Opcode::CondJumpNot => {
                let taken = truthy(&regs[usize::from(instr.u8(0))]);
                let want = op == Opcode::CondJump;
                if taken == want {
                    ip_index = jump_target(instr, next_index, &offsets, instr.i32(1))?;
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
                if cap_id == 1 && method_id == 1 {
                    let arg = match &regs[usize::from(args_reg)] {
                        Value::Record(fields) if !fields.is_empty() => fields[0].1.clone(),
                        _ => return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset)),
                    };
                    signals.write(99, arg.clone());
                    regs[usize::from(result_reg)] = arg;
                } else {
                    return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset));
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
                    ip_index = jump_target(instr, next_index, &offsets, instr.i32(5))?;
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
                if gas < budget {
                    return Err(VmError::at(VmErrorKind::GasExhausted, instr.offset));
                }
            }
            Opcode::Halt => break,
            // `Opcode` is `#[non_exhaustive]` in flux-syntax; future opcodes are
            // unreachable here because the decoder only emits known variants and
            // unknown bytes are rejected at decode time (InvalidDispatch).
            _ => unreachable!("decoder yields only known opcodes: {:?}", op),
        }
        ip_index = next_index;
    }

    let out_signals = signals.snapshot();
    Ok(VmOutcome {
        signals: out_signals,
        registers: regs,
        gas_used: ENTRY_GAS - gas,
    })
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

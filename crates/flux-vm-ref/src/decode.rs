//! Bytecode decoder for the reference VM.
//!
//! Decoding is total: any unassigned opcode byte yields an `InvalidDispatch`
//! error rather than an undefined variant, because this crate forbids
//! `unsafe_code` and must never transmute a byte into an opcode. The decoder is
//! monomorphized-agnostic — it does not interpret operand *meaning*, only layout,
//! using [`Opcode::operand_len`] (derived from Appendix E §E.1).

use flux_syntax::opcode::Opcode;

use crate::error::{VmError, VmErrorKind};

/// A decoded instruction: its opcode and the raw operand bytes that follow it.
///
/// Operands are kept as raw little-endian byte slices so the interpreter can
/// extract exactly the widths each opcode expects, without a per-instruction
/// heap allocation in the hot path (AGENTS.md §3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// The decoded opcode.
    pub opcode: Opcode,
    /// Byte offset of this instruction within the program (for diagnostics).
    pub offset: u32,
    /// Raw operand bytes (length == `opcode.operand_len()`).
    pub operands: [u8; 9],
    /// Number of valid bytes in `operands`.
    pub operand_len: u8,
}

impl Instruction {
    /// Reads a `u8` operand at `index` (0-based within the operand bytes).
    #[must_use]
    pub const fn u8(&self, index: usize) -> u8 {
        self.operands[index]
    }

    /// Reads a little-endian `u16` operand starting at `index`.
    #[must_use]
    pub const fn u16(&self, index: usize) -> u16 {
        u16::from_le_bytes([self.operands[index], self.operands[index + 1]])
    }

    /// Reads a little-endian `u32` operand starting at `index`.
    #[must_use]
    pub const fn u32(&self, index: usize) -> u32 {
        u32::from_le_bytes([
            self.operands[index],
            self.operands[index + 1],
            self.operands[index + 2],
            self.operands[index + 3],
        ])
    }

    /// Reads a little-endian `i32` operand starting at `index`.
    #[must_use]
    pub const fn i32(&self, index: usize) -> i32 {
        i32::from_le_bytes([
            self.operands[index],
            self.operands[index + 1],
            self.operands[index + 2],
            self.operands[index + 3],
        ])
    }

    /// Reads a little-endian `i64` operand starting at `index`.
    #[must_use]
    pub const fn i64(&self, index: usize) -> i64 {
        i64::from_le_bytes([
            self.operands[index],
            self.operands[index + 1],
            self.operands[index + 2],
            self.operands[index + 3],
            self.operands[index + 4],
            self.operands[index + 5],
            self.operands[index + 6],
            self.operands[index + 7],
        ])
    }

    /// Reads a little-endian `f64` operand starting at `index`.
    #[must_use]
    pub const fn f64(&self, index: usize) -> f64 {
        f64::from_le_bytes([
            self.operands[index],
            self.operands[index + 1],
            self.operands[index + 2],
            self.operands[index + 3],
            self.operands[index + 4],
            self.operands[index + 5],
            self.operands[index + 6],
            self.operands[index + 7],
        ])
    }
}

/// Size of the register file (`[Value; 16]`, Appendix E §E.1). Register
/// operand bytes must index below this or every handler's `regs[usize::from(..)]`
/// would panic with an index-out-of-bounds instead of surfacing a `VmError`.
const REGISTER_FILE_SIZE: u8 = 16;

/// Returns the operand-byte positions that hold register indices for `opcode`.
///
/// Every position listed here is read as `regs[usize::from(instr.u8(pos))]` by
/// the interpreter. Most opcodes keep registers in their leading `u8`s; the
/// exceptions are the `REG_U16_REG` shapes (`GET_FIELD`/`SET_FIELD`/
/// `EXTRACT_FIELD`: trailing source register at position 3), `CALL_CAP`
/// (`reg, u32, u16, reg`: trailing args register at position 7) and
/// `WRITE_SIGNAL` (`u32 signal id, u8 src`: register at position 4).
fn register_operand_positions(opcode: Opcode) -> &'static [usize] {
    match opcode {
        Opcode::Halt | Opcode::Nop | Opcode::Jump | Opcode::GasCheck => &[],
        Opcode::LoadNull
        | Opcode::AllocRecord
        | Opcode::AllocList
        | Opcode::LoadBoolConst
        | Opcode::LoadIntConst
        | Opcode::LoadFloatConst
        | Opcode::LoadStrConst
        | Opcode::StrIntern
        | Opcode::ListClear
        | Opcode::ReadSignal
        | Opcode::CondJump
        | Opcode::CondJumpNot
        | Opcode::MatchTag => &[0],
        Opcode::NegI64
        | Opcode::NegF64
        | Opcode::I64ToF64
        | Opcode::F64ToI64
        | Opcode::NotBool
        | Opcode::StrLen
        | Opcode::Mov
        | Opcode::ToString
        | Opcode::ListLen
        | Opcode::ListRemove
        | Opcode::ListRemoveItem
        | Opcode::IsNull
        | Opcode::ListPush
        | Opcode::Await => &[0, 1],
        Opcode::AddI64
        | Opcode::SubI64
        | Opcode::MulI64
        | Opcode::DivI64
        | Opcode::ModI64
        | Opcode::EqI64
        | Opcode::LtI64
        | Opcode::GtI64
        | Opcode::LteI64
        | Opcode::GteI64
        | Opcode::AddF64
        | Opcode::SubF64
        | Opcode::MulF64
        | Opcode::DivF64
        | Opcode::EqF64
        | Opcode::LtF64
        | Opcode::GtF64
        | Opcode::AndBool
        | Opcode::OrBool
        | Opcode::StrConcat
        | Opcode::StrEq
        | Opcode::RecordEq
        | Opcode::ListGet
        | Opcode::ListConcat
        | Opcode::ListInsert
        | Opcode::BoolEq => &[0, 1, 2],
        Opcode::WriteSignal => &[4],
        Opcode::GetField | Opcode::SetField | Opcode::ExtractField => &[0, 3],
        Opcode::CallCap => &[0, 7],
        // `Opcode` is `#[non_exhaustive]` in `flux-syntax`, so a foreign match
        // needs a wildcard arm even with every current variant covered.
        // Unreachable in practice: `Opcode::from_byte` only yields known
        // variants, and any *new* opcode must update this table with its real
        // register positions.
        _ => &[],
    }
}

/// Decodes a whole program into a vector of instructions.
///
/// # Errors
///
/// Returns [`VmErrorKind::InvalidDispatch`] at the first byte that is not a valid
/// opcode, [`VmErrorKind::IndexOutOfBounds`] if the program is truncated
/// (an instruction's operand bytes run past the end of the buffer) or if a
/// register operand indexes at or beyond the 16-register file.
pub fn decode_program(bytes: &[u8]) -> Result<Vec<Instruction>, VmError> {
    let mut instrs = Vec::with_capacity(bytes.len() / 2);
    let mut ip = 0usize;
    while ip < bytes.len() {
        let offset = u32::try_from(ip).unwrap_or(u32::MAX);
        let opcode = match Opcode::from_byte(bytes[ip]) {
            Some(op) => op,
            None => return Err(VmError::at(VmErrorKind::InvalidDispatch, offset)),
        };
        let n = opcode.operand_len() as usize;
        let start = ip + 1;
        let end = start + n;
        if end > bytes.len() {
            return Err(VmError::at(VmErrorKind::IndexOutOfBounds, offset));
        }
        let mut operands = [0u8; 9];
        operands[..n].copy_from_slice(&bytes[start..end]);
        // Validate register operands up front so `exec_tail` can index `regs`
        // without re-checking: a crafted or corrupted bytecode blob must fault
        // with a `VmError`, never panic the interpreter (fuzz charter).
        for &pos in register_operand_positions(opcode) {
            if operands[pos] >= REGISTER_FILE_SIZE {
                return Err(VmError::at(VmErrorKind::IndexOutOfBounds, offset));
            }
        }
        instrs.push(Instruction {
            opcode,
            offset,
            operands,
            operand_len: n as u8,
        });
        ip = end;
    }
    Ok(instrs)
}

//! VM opcode vocabulary, normative per Appendix E §E.1.
//!
//! The instruction set is intentionally minimal and **monomorphized**: there is
//! no generic `ADD` with runtime tag dispatch, only type-specific `ADD_I64` and
//! `ADD_F64`. The type checker proves operand types before lowering, so the VM
//! never inspects a tag to choose an arithmetic implementation.
//!
//! These byte values are a wire contract. The production VMs are native Swift
//! and Kotlin (ADR-0002) and declare their own constants from the same table, so
//! any change here must be mirrored in all three implementations and in the
//! golden ISA vectors under `/tests/isa-vectors/`. Adding an opcode requires an
//! ADR.
//!
//! # Examples
//!
//! ```
//! use flux_syntax::opcode::{self, Opcode};
//!
//! // Decode a byte from a bytecode stream.
//! let op = Opcode::from_byte(opcode::ADD_I64).expect("0x20 is ADD_I64");
//! assert_eq!(op.mnemonic(), "ADD_I64");
//!
//! // Advance the program counter past the whole instruction.
//! assert_eq!(op.instruction_len(), 4); // opcode + dst + a + b
//! ```

mod decode;
pub mod raw;
mod width;

pub use raw::*;

/// A decoded VM instruction opcode.
///
/// Decoding is total: [`Opcode::from_byte`] returns `None` for any unassigned
/// byte rather than producing an invalid variant, so a corrupt or
/// future-versioned frame is reported as a protocol error instead of
/// triggering undefined behaviour. This crate is `#![forbid(unsafe_code)]`, so
/// no `transmute`-based decoding is possible here by construction.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Opcode {
    /// `HALT` — stop execution.
    Halt = raw::HALT,
    /// `NOP` — no operation.
    Nop = raw::NOP,
    /// `READ_SIGNAL` — read a signal into a register.
    ReadSignal = raw::READ_SIGNAL,
    /// `WRITE_SIGNAL` — write a register into a signal.
    WriteSignal = raw::WRITE_SIGNAL,
    /// `ADD_I64` — integer addition.
    AddI64 = raw::ADD_I64,
    /// `SUB_I64` — integer subtraction.
    SubI64 = raw::SUB_I64,
    /// `MUL_I64` — integer multiplication.
    MulI64 = raw::MUL_I64,
    /// `DIV_I64` — integer division.
    DivI64 = raw::DIV_I64,
    /// `MOD_I64` — integer remainder.
    ModI64 = raw::MOD_I64,
    /// `NEG_I64` — integer negation.
    NegI64 = raw::NEG_I64,
    /// `EQ_I64` — integer equality.
    EqI64 = raw::EQ_I64,
    /// `LT_I64` — integer less-than.
    LtI64 = raw::LT_I64,
    /// `GT_I64` — integer greater-than.
    GtI64 = raw::GT_I64,
    /// `LTE_I64` — integer less-than-or-equal.
    LteI64 = raw::LTE_I64,
    /// `GTE_I64` — integer greater-than-or-equal.
    GteI64 = raw::GTE_I64,
    /// `ADD_F64` — float addition.
    AddF64 = raw::ADD_F64,
    /// `SUB_F64` — float subtraction.
    SubF64 = raw::SUB_F64,
    /// `MUL_F64` — float multiplication.
    MulF64 = raw::MUL_F64,
    /// `DIV_F64` — float division.
    DivF64 = raw::DIV_F64,
    /// `NEG_F64` — float negation.
    NegF64 = raw::NEG_F64,
    /// `EQ_F64` — float equality.
    EqF64 = raw::EQ_F64,
    /// `LT_F64` — float less-than.
    LtF64 = raw::LT_F64,
    /// `GT_F64` — float greater-than.
    GtF64 = raw::GT_F64,
    /// `I64_TO_F64` — widen integer to float.
    I64ToF64 = raw::I64_TO_F64,
    /// `F64_TO_I64` — truncate float to integer.
    F64ToI64 = raw::F64_TO_I64,
    /// `AND_BOOL` — boolean conjunction.
    AndBool = raw::AND_BOOL,
    /// `OR_BOOL` — boolean disjunction.
    OrBool = raw::OR_BOOL,
    /// `NOT_BOOL` — boolean negation.
    NotBool = raw::NOT_BOOL,
    /// `BOOL_EQ` — boolean equality (`==` / `!=` over `Bool` operands).
    BoolEq = raw::BOOL_EQ,
    /// `STR_CONCAT` — string concatenation.
    StrConcat = raw::STR_CONCAT,
    /// `STR_INTERN` — intern a string literal.
    StrIntern = raw::STR_INTERN,
    /// `STR_EQ` — string equality.
    StrEq = raw::STR_EQ,
    /// `STR_LEN` — string length in bytes.
    StrLen = raw::STR_LEN,
    /// `JUMP` — unconditional relative jump.
    Jump = raw::JUMP,
    /// `COND_JUMP` — jump when truthy.
    CondJump = raw::COND_JUMP,
    /// `COND_JUMP_NOT` — jump when falsy.
    CondJumpNot = raw::COND_JUMP_NOT,
    /// `ALLOC_RECORD` — allocate a record.
    AllocRecord = raw::ALLOC_RECORD,
    /// `GET_FIELD` — read a record field.
    GetField = raw::GET_FIELD,
    /// `SET_FIELD` — write a record field.
    SetField = raw::SET_FIELD,
    /// `RECORD_EQ` — structural record equality.
    RecordEq = raw::RECORD_EQ,
    /// `ALLOC_LIST` — allocate a persistent list.
    AllocList = raw::ALLOC_LIST,
    /// `LIST_PUSH` — append, yielding a new list.
    ListPush = raw::LIST_PUSH,
    /// `LIST_GET` — index into a list.
    ListGet = raw::LIST_GET,
    /// `LIST_LEN` — list length.
    ListLen = raw::LIST_LEN,
    /// `LIST_CONCAT` — list concatenation.
    ListConcat = raw::LIST_CONCAT,
    /// `LIST_INSERT` — insert `val` into `list` at `idx`.
    ListInsert = raw::LIST_INSERT,
    /// `LIST_REMOVE` — remove the element at `idx` from `list`.
    ListRemove = raw::LIST_REMOVE,
    /// `LIST_CLEAR` — clear `list`, leaving it empty.
    ListClear = raw::LIST_CLEAR,
    /// `LIST_REMOVE_ITEM` — remove the first element equal to `val`.
    ListRemoveItem = raw::LIST_REMOVE_ITEM,
    /// `CALL_CAP` — invoke a host capability.
    CallCap = raw::CALL_CAP,
    /// `MATCH_TAG` — jump on variant tag match.
    MatchTag = raw::MATCH_TAG,
    /// `EXTRACT_FIELD` — extract a variant field.
    ExtractField = raw::EXTRACT_FIELD,
    /// `LOAD_INT_CONST` — load an `i64` immediate.
    LoadIntConst = raw::LOAD_INT_CONST,
    /// `LOAD_FLOAT_CONST` — load an `f64` immediate.
    LoadFloatConst = raw::LOAD_FLOAT_CONST,
    /// `LOAD_BOOL_CONST` — load a `bool` immediate.
    LoadBoolConst = raw::LOAD_BOOL_CONST,
    /// `LOAD_STR_CONST` — load an interned string.
    LoadStrConst = raw::LOAD_STR_CONST,
    /// `LOAD_NULL` — load `Null`.
    LoadNull = raw::LOAD_NULL,
    /// `MOV` — copy between registers.
    Mov = raw::MOV,
    /// `GAS_CHECK` — assert remaining gas covers a budget.
    GasCheck = raw::GAS_CHECK,
    /// `TO_STRING` — convert any value to its interned string form (ADR-0043).
    ToString = raw::TO_STRING,
    /// `AWAIT` — suspend the VM, capturing the continuation (ADR-0044, MLP v2).
    Await = raw::AWAIT,
    /// `IS_NULL` — test whether a register holds `Null` (used to short-circuit
    /// optional field reads; see `raw::IS_NULL` and its doc note).
    IsNull = raw::IS_NULL,
}

impl Opcode {
    /// Every opcode defined by Appendix E §E.1, in ascending byte order.
    ///
    /// Useful for exhaustive conformance tests and disassembler tables.
    pub const ALL: [Self; 61] = [
        Self::Halt,
        Self::Nop,
        Self::ReadSignal,
        Self::WriteSignal,
        Self::AddI64,
        Self::SubI64,
        Self::MulI64,
        Self::DivI64,
        Self::ModI64,
        Self::NegI64,
        Self::EqI64,
        Self::LtI64,
        Self::GtI64,
        Self::LteI64,
        Self::GteI64,
        Self::AddF64,
        Self::SubF64,
        Self::MulF64,
        Self::DivF64,
        Self::NegF64,
        Self::EqF64,
        Self::LtF64,
        Self::GtF64,
        Self::I64ToF64,
        Self::F64ToI64,
        Self::AndBool,
        Self::OrBool,
        Self::NotBool,
        Self::StrConcat,
        Self::StrIntern,
        Self::StrEq,
        Self::StrLen,
        Self::Jump,
        Self::CondJump,
        Self::CondJumpNot,
        Self::AllocRecord,
        Self::GetField,
        Self::SetField,
        Self::RecordEq,
        Self::AllocList,
        Self::ListPush,
        Self::ListGet,
        Self::ListLen,
        Self::ListConcat,
        // --- FLUX-072: dynamic-list mutation opcodes. These were implemented in
        // `flux-vm-ref` and ported to both host VMs, but `ALL` was never extended
        // to include them — so `ALL` (the canonical opcode contract) was stale
        // and drifted from the hosts. Re-added here (FLUX-078).
        Self::ListInsert,
        Self::ListRemove,
        Self::ListClear,
        Self::ListRemoveItem,
        Self::CallCap,
        Self::MatchTag,
        Self::ExtractField,
        Self::LoadIntConst,
        Self::LoadFloatConst,
        Self::LoadBoolConst,
        Self::LoadStrConst,
        Self::LoadNull,
        Self::Mov,
        Self::GasCheck,
        Self::ToString,
        Self::Await,
        Self::IsNull,
    ];
}

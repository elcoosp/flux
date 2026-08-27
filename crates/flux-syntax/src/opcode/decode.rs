//! Decoding, width and mnemonic tables for [`Opcode`].
//!
//! Split out of the module root to keep each file within the 300-line limit
//! (AGENTS.md §1.2). These are pure lookup tables over Appendix E §E.1.

use super::{Opcode, raw, width};

impl Opcode {
    /// Decodes an opcode byte, returning `None` for any unassigned value.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_syntax::opcode::Opcode;
    ///
    /// assert_eq!(Opcode::from_byte(0x00), Some(Opcode::Halt));
    /// assert_eq!(Opcode::from_byte(0xFF), None);
    /// ```
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        let opcode = match byte {
            raw::HALT => Self::Halt,
            raw::NOP => Self::Nop,
            raw::READ_SIGNAL => Self::ReadSignal,
            raw::WRITE_SIGNAL => Self::WriteSignal,
            raw::ADD_I64 => Self::AddI64,
            raw::SUB_I64 => Self::SubI64,
            raw::MUL_I64 => Self::MulI64,
            raw::DIV_I64 => Self::DivI64,
            raw::MOD_I64 => Self::ModI64,
            raw::NEG_I64 => Self::NegI64,
            raw::EQ_I64 => Self::EqI64,
            raw::LT_I64 => Self::LtI64,
            raw::GT_I64 => Self::GtI64,
            raw::LTE_I64 => Self::LteI64,
            raw::GTE_I64 => Self::GteI64,
            raw::ADD_F64 => Self::AddF64,
            raw::SUB_F64 => Self::SubF64,
            raw::MUL_F64 => Self::MulF64,
            raw::DIV_F64 => Self::DivF64,
            raw::NEG_F64 => Self::NegF64,
            raw::EQ_F64 => Self::EqF64,
            raw::LT_F64 => Self::LtF64,
            raw::GT_F64 => Self::GtF64,
            raw::I64_TO_F64 => Self::I64ToF64,
            raw::F64_TO_I64 => Self::F64ToI64,
            raw::AND_BOOL => Self::AndBool,
            raw::OR_BOOL => Self::OrBool,
            raw::NOT_BOOL => Self::NotBool,
            raw::STR_CONCAT => Self::StrConcat,
            raw::STR_INTERN => Self::StrIntern,
            raw::STR_EQ => Self::StrEq,
            raw::STR_LEN => Self::StrLen,
            raw::JUMP => Self::Jump,
            raw::COND_JUMP => Self::CondJump,
            raw::COND_JUMP_NOT => Self::CondJumpNot,
            raw::ALLOC_RECORD => Self::AllocRecord,
            raw::GET_FIELD => Self::GetField,
            raw::SET_FIELD => Self::SetField,
            raw::RECORD_EQ => Self::RecordEq,
            raw::ALLOC_LIST => Self::AllocList,
            raw::LIST_PUSH => Self::ListPush,
            raw::LIST_GET => Self::ListGet,
            raw::LIST_LEN => Self::ListLen,
            raw::LIST_CONCAT => Self::ListConcat,
            raw::CALL_CAP => Self::CallCap,
            raw::MATCH_TAG => Self::MatchTag,
            raw::EXTRACT_FIELD => Self::ExtractField,
            raw::LOAD_INT_CONST => Self::LoadIntConst,
            raw::LOAD_FLOAT_CONST => Self::LoadFloatConst,
            raw::LOAD_BOOL_CONST => Self::LoadBoolConst,
            raw::LOAD_STR_CONST => Self::LoadStrConst,
            raw::LOAD_NULL => Self::LoadNull,
            raw::MOV => Self::Mov,
            raw::GAS_CHECK => Self::GasCheck,
            raw::TO_STRING => Self::ToString,
            raw::AWAIT => Self::Await,
            _ => return None,
        };
        Some(opcode)
    }

    /// Returns the opcode's byte encoding.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        self as u8
    }

    /// Returns the number of operand bytes that follow this opcode.
    ///
    /// Adding this to 1 (the opcode byte itself) gives the total instruction
    /// width; [`Opcode::instruction_len`] does that for you.
    #[must_use]
    pub const fn operand_len(self) -> u8 {
        match self {
            Self::Halt | Self::Nop => width::NONE,
            Self::LoadNull => width::REG,
            Self::NegI64
            | Self::NegF64
            | Self::I64ToF64
            | Self::F64ToI64
            | Self::NotBool
            | Self::StrLen
            | Self::Mov
            | Self::ToString
            | Self::ListLen => width::REG_REG,
            Self::AddI64
            | Self::SubI64
            | Self::MulI64
            | Self::DivI64
            | Self::ModI64
            | Self::EqI64
            | Self::LtI64
            | Self::GtI64
            | Self::LteI64
            | Self::GteI64
            | Self::AddF64
            | Self::SubF64
            | Self::MulF64
            | Self::DivF64
            | Self::EqF64
            | Self::LtF64
            | Self::GtF64
            | Self::AndBool
            | Self::OrBool
            | Self::StrConcat
            | Self::StrEq
            | Self::RecordEq
            | Self::ListGet
            | Self::ListConcat => width::REG_REG_REG,
            Self::ReadSignal | Self::WriteSignal | Self::CondJump | Self::CondJumpNot => {
                width::REG_U32
            }
            Self::StrIntern | Self::LoadStrConst => width::REG_U32,
            Self::AllocRecord | Self::AllocList => width::REG_U16,
            Self::LoadBoolConst | Self::ListPush => width::REG_U8,
            Self::LoadIntConst | Self::LoadFloatConst => width::REG_I64,
            Self::Jump => width::I32,
            Self::GasCheck => width::U32,
            Self::GetField => width::REG_U16_REG,
            Self::SetField | Self::ExtractField => width::REG_U16_REG,
            Self::MatchTag => width::REG_U32_I32,
            Self::CallCap => width::CALL_CAP,
            Self::Await => width::AWAIT,
        }
    }

    /// Returns the total instruction width in bytes, including the opcode byte.
    #[must_use]
    pub const fn instruction_len(self) -> u8 {
        self.operand_len() + 1
    }

    /// Returns the Appendix E mnemonic, e.g. `"ADD_I64"`.
    #[must_use]
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Halt => "HALT",
            Self::Nop => "NOP",
            Self::ReadSignal => "READ_SIGNAL",
            Self::WriteSignal => "WRITE_SIGNAL",
            Self::AddI64 => "ADD_I64",
            Self::SubI64 => "SUB_I64",
            Self::MulI64 => "MUL_I64",
            Self::DivI64 => "DIV_I64",
            Self::ModI64 => "MOD_I64",
            Self::NegI64 => "NEG_I64",
            Self::EqI64 => "EQ_I64",
            Self::LtI64 => "LT_I64",
            Self::GtI64 => "GT_I64",
            Self::LteI64 => "LTE_I64",
            Self::GteI64 => "GTE_I64",
            Self::AddF64 => "ADD_F64",
            Self::SubF64 => "SUB_F64",
            Self::MulF64 => "MUL_F64",
            Self::DivF64 => "DIV_F64",
            Self::NegF64 => "NEG_F64",
            Self::EqF64 => "EQ_F64",
            Self::LtF64 => "LT_F64",
            Self::GtF64 => "GT_F64",
            Self::I64ToF64 => "I64_TO_F64",
            Self::F64ToI64 => "F64_TO_I64",
            Self::AndBool => "AND_BOOL",
            Self::OrBool => "OR_BOOL",
            Self::NotBool => "NOT_BOOL",
            Self::StrConcat => "STR_CONCAT",
            Self::StrIntern => "STR_INTERN",
            Self::StrEq => "STR_EQ",
            Self::StrLen => "STR_LEN",
            Self::Jump => "JUMP",
            Self::CondJump => "COND_JUMP",
            Self::CondJumpNot => "COND_JUMP_NOT",
            Self::AllocRecord => "ALLOC_RECORD",
            Self::GetField => "GET_FIELD",
            Self::SetField => "SET_FIELD",
            Self::RecordEq => "RECORD_EQ",
            Self::AllocList => "ALLOC_LIST",
            Self::ListPush => "LIST_PUSH",
            Self::ListGet => "LIST_GET",
            Self::ListLen => "LIST_LEN",
            Self::ListConcat => "LIST_CONCAT",
            Self::CallCap => "CALL_CAP",
            Self::MatchTag => "MATCH_TAG",
            Self::ExtractField => "EXTRACT_FIELD",
            Self::LoadIntConst => "LOAD_INT_CONST",
            Self::LoadFloatConst => "LOAD_FLOAT_CONST",
            Self::LoadBoolConst => "LOAD_BOOL_CONST",
            Self::LoadStrConst => "LOAD_STR_CONST",
            Self::LoadNull => "LOAD_NULL",
            Self::Mov => "MOV",
            Self::GasCheck => "GAS_CHECK",
            Self::ToString => "TO_STRING",
            Self::Await => "AWAIT",
        }
    }
}

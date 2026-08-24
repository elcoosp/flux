//! Operand widths in bytes, by operand shape.
//!
//! Taken from the "Args (bytes)" column of Appendix E §E.1. Named shapes keep
//! the width table readable and prevent magic numbers in
//! [`Opcode::operand_len`](super::Opcode::operand_len).
/// No operands.
pub(crate) const NONE: u8 = 0;
/// `dst(u8)`.
pub(crate) const REG: u8 = 1;
/// `dst(u8), src(u8)`.
pub(crate) const REG_REG: u8 = 2;
/// `dst(u8), a(u8), b(u8)`.
pub(crate) const REG_REG_REG: u8 = 3;
/// `reg(u8), u32`.
pub(crate) const REG_U32: u8 = 5;
/// `reg(u8), u16`.
pub(crate) const REG_U16: u8 = 3;
/// `reg(u8), u8`.
pub(crate) const REG_U8: u8 = 2;
/// `reg(u8), i64` or `reg(u8), f64`.
pub(crate) const REG_I64: u8 = 9;
/// `i32` jump offset.
pub(crate) const I32: u8 = 4;
/// `u32` immediate.
pub(crate) const U32: u8 = 4;
/// `reg(u8), u16, reg(u8)`.
pub(crate) const REG_U16_REG: u8 = 4;
/// `dst(u8), u32, i32`.
pub(crate) const REG_U32_I32: u8 = 9;
/// `reg(u8), u32, u16, reg(u8)`.
pub(crate) const CALL_CAP: u8 = 8;

//! Raw opcode byte values, exactly as tabulated in Appendix E §E.1.
//!
//! Prefer [`Opcode`](super::Opcode) in new code; these constants exist for
//! bytecode emitters and decoders that work in raw bytes. The values are a wire
//! contract shared with the native Swift and Kotlin VMs — see the module docs on
//! [`opcode`](super) before changing anything here.
/// Stop execution.
pub const HALT: u8 = 0x00;
/// No operation.
pub const NOP: u8 = 0x01;

/// Read a signal's current value into a register.
pub const READ_SIGNAL: u8 = 0x10;
/// Write a register's value into a signal, marking it dirty.
pub const WRITE_SIGNAL: u8 = 0x11;

/// Integer addition.
pub const ADD_I64: u8 = 0x20;
/// Integer subtraction.
pub const SUB_I64: u8 = 0x21;
/// Integer multiplication.
pub const MUL_I64: u8 = 0x22;
/// Integer division.
pub const DIV_I64: u8 = 0x23;
/// Integer remainder.
pub const MOD_I64: u8 = 0x24;
/// Integer negation.
pub const NEG_I64: u8 = 0x25;
/// Integer equality.
pub const EQ_I64: u8 = 0x26;
/// Integer less-than.
pub const LT_I64: u8 = 0x27;
/// Integer greater-than.
pub const GT_I64: u8 = 0x28;
/// Integer less-than-or-equal.
pub const LTE_I64: u8 = 0x29;
/// Integer greater-than-or-equal.
pub const GTE_I64: u8 = 0x2A;

/// Float addition.
pub const ADD_F64: u8 = 0x30;
/// Float subtraction.
pub const SUB_F64: u8 = 0x31;
/// Float multiplication.
pub const MUL_F64: u8 = 0x32;
/// Float division.
pub const DIV_F64: u8 = 0x33;
/// Float negation.
pub const NEG_F64: u8 = 0x34;
/// Float equality.
pub const EQ_F64: u8 = 0x35;
/// Float less-than.
pub const LT_F64: u8 = 0x36;
/// Float greater-than.
pub const GT_F64: u8 = 0x37;
/// Widen an integer to a float.
pub const I64_TO_F64: u8 = 0x38;
/// Truncate a float to an integer.
pub const F64_TO_I64: u8 = 0x39;

/// Boolean conjunction.
pub const AND_BOOL: u8 = 0x40;
/// Boolean disjunction.
pub const OR_BOOL: u8 = 0x41;
/// Boolean negation.
pub const NOT_BOOL: u8 = 0x42;
/// Boolean equality (`==` / `!=` over `Bool` operands).
pub const BOOL_EQ: u8 = 0x43;

/// String concatenation.
pub const STR_CONCAT: u8 = 0x50;
/// Intern a string literal from the string table.
pub const STR_INTERN: u8 = 0x51;
/// String equality.
pub const STR_EQ: u8 = 0x52;
/// String length in bytes.
pub const STR_LEN: u8 = 0x53;

/// Unconditional relative jump.
pub const JUMP: u8 = 0x60;
/// Relative jump taken when the register is truthy.
pub const COND_JUMP: u8 = 0x61;
/// Relative jump taken when the register is falsy.
pub const COND_JUMP_NOT: u8 = 0x62;

/// Allocate a record with a fixed field count.
pub const ALLOC_RECORD: u8 = 0x70;
/// Read a record field by index.
pub const GET_FIELD: u8 = 0x71;
/// Write a record field by index.
pub const SET_FIELD: u8 = 0x72;
/// Structural record equality.
pub const RECORD_EQ: u8 = 0x73;

/// Allocate a persistent list with a capacity hint.
pub const ALLOC_LIST: u8 = 0x80;
/// Append to a list, yielding a new persistent list.
pub const LIST_PUSH: u8 = 0x81;
/// Index into a list.
pub const LIST_GET: u8 = 0x82;
/// List length.
pub const LIST_LEN: u8 = 0x83;
/// List concatenation.
pub const LIST_CONCAT: u8 = 0x84;
/// Insert `val` into `list` at `idx` (shifting later elements right).
pub const LIST_INSERT: u8 = 0x85;
/// Remove the element at `idx` from `list`, returning the shortened list.
pub const LIST_REMOVE: u8 = 0x86;
/// Clear `list`, leaving an empty list (same identity, length 0).
pub const LIST_CLEAR: u8 = 0x87;
/// Remove the first element of `list` equal to `val` (by value equality).
pub const LIST_REMOVE_ITEM: u8 = 0x88;

/// Invoke a host capability; the result arrives via a callback handler.
pub const CALL_CAP: u8 = 0x90;

/// Jump when a variant's tag matches.
pub const MATCH_TAG: u8 = 0xA0;
/// Extract a field from a matched variant.
pub const EXTRACT_FIELD: u8 = 0xA1;

/// Load an `i64` immediate.
pub const LOAD_INT_CONST: u8 = 0xB0;
/// Load an `f64` immediate.
pub const LOAD_FLOAT_CONST: u8 = 0xB1;
/// Load a `bool` immediate.
pub const LOAD_BOOL_CONST: u8 = 0xB2;
/// Load an interned string by ID.
pub const LOAD_STR_CONST: u8 = 0xB3;
/// Load `Null`.
pub const LOAD_NULL: u8 = 0xB4;
/// Copy one register to another.
pub const MOV: u8 = 0xB5;

/// Fail with `GasExhausted` unless the remaining gas covers the budget.
pub const GAS_CHECK: u8 = 0xC0;

/// Convert any value to its interned string representation (ADR-0043).
pub const TO_STRING: u8 = 0xD0;

/// Suspend the VM, capturing the continuation (ADR-0044, MLP v2 first-class async).
///
/// Operands: `result_reg(u8), future_reg(u8)`. On execute the interpreter snapshots
/// its live state (`ip`, registers, remaining gas, captured signals) and returns a
/// `Suspended` result instead of `Halt`. The executor arranges for `future_reg`'s
/// value to be delivered back, then calls `resume` with that value to continue.
pub const AWAIT: u8 = 0xE0;

/// Test whether a register holds `Null` (FLUX-053 null-safe access).
///
/// Operands: `dst(u8), src(u8)`. Sets `dst` to `true` when `src` is the
/// `Null` value, `false` otherwise. Optional chaining (`base?.field`) lowers to
/// an `IS_NULL` test that short-circuits the field read. This is the one
/// null-distinguishing primitive the VM was missing: `truthy` treats both
/// `Null` and `Int(0)` as falsey, so it cannot discriminate a present `0` from
/// an absent `Null`. Native Swift/Kotlin VMs mirror this opcode (ADR pending).
pub const IS_NULL: u8 = 0xD1;

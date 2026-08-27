//! Behavioural tests for the shared VM opcode vocabulary.
//!
//! The opcode constants are normative: Appendix E §E.1 fixes each byte value,
//! and the Swift and Kotlin VMs declare their own constants from the same
//! table. A drift here silently breaks every host, so these tests pin the
//! encoding rather than the implementation.

use flux_syntax::opcode::{self, Opcode};

// === Byte values (Appendix E §E.1) ===

#[test]
fn test_halt_encodes_as_zero() {
    assert_eq!(opcode::HALT, 0x00);
}

#[test]
fn test_signal_opcodes_match_appendix_e() {
    assert_eq!((opcode::READ_SIGNAL, opcode::WRITE_SIGNAL), (0x10, 0x11));
}

#[test]
fn test_integer_arithmetic_opcodes_match_appendix_e() {
    assert_eq!(
        (opcode::ADD_I64, opcode::GTE_I64),
        (0x20, 0x2A),
        "integer block spans 0x20..=0x2A"
    );
}

#[test]
fn test_float_arithmetic_opcodes_match_appendix_e() {
    assert_eq!((opcode::ADD_F64, opcode::F64_TO_I64), (0x30, 0x39));
}

#[test]
fn test_gas_check_encodes_as_c0() {
    assert_eq!(opcode::GAS_CHECK, 0xC0);
}

// === Total decoding ===

#[test]
fn test_opcode_from_byte_decodes_a_known_opcode() {
    assert_eq!(Opcode::from_byte(0x20), Some(Opcode::AddI64));
}

#[test]
fn test_opcode_from_byte_rejects_an_unassigned_byte() {
    assert_eq!(
        Opcode::from_byte(0xFF),
        None,
        "a corrupt or future-version frame must decode to None, never to an invalid opcode"
    );
}

#[test]
fn test_opcode_round_trips_through_its_byte() {
    let halt = Opcode::from_byte(opcode::HALT);
    assert_eq!(halt.map(Opcode::to_byte), Some(opcode::HALT));
}

// === Operand widths (Appendix E §E.1 "Args (bytes)") ===

#[test]
fn test_halt_takes_no_operands() {
    assert_eq!(Opcode::Halt.operand_len(), 0);
}

#[test]
fn test_read_signal_takes_five_operand_bytes() {
    assert_eq!(
        Opcode::ReadSignal.operand_len(),
        5,
        "reg_dst(u8) + signal_id(u32)"
    );
}

#[test]
fn test_binary_integer_op_takes_three_register_operands() {
    assert_eq!(Opcode::AddI64.operand_len(), 3);
}

#[test]
fn test_load_int_const_takes_nine_operand_bytes() {
    assert_eq!(
        Opcode::LoadIntConst.operand_len(),
        9,
        "dst(u8) + value(i64)"
    );
}

#[test]
fn test_call_cap_takes_eight_operand_bytes() {
    assert_eq!(
        Opcode::CallCap.operand_len(),
        8,
        "result_reg(u8) + cap_id(u32) + method_id(u16) + args_reg(u8)"
    );
}

#[test]
fn test_instruction_len_includes_the_opcode_byte() {
    assert_eq!(Opcode::AddI64.instruction_len(), 4);
}

// === Coverage ===

#[test]
fn test_every_opcode_in_appendix_e_is_declared() {
    assert_eq!(
        Opcode::ALL.len(),
        56,
        "Appendix E §E.1 + ADR-0043 `TO_STRING` + ADR-0044 `AWAIT` define 56 opcodes; update together"
    );
}

#[test]
fn test_await_decodes_from_its_byte_with_two_register_operands() {
    let op = Opcode::from_byte(0xE0).expect("AWAIT is assigned byte 0xE0 (ADR-0044)");
    assert_eq!(op, Opcode::Await);
    assert_eq!(op.mnemonic(), "AWAIT");
    assert_eq!(op.operand_len(), 2, "AWAIT takes result_reg(u8), future_reg(u8)");
}

#[test]
fn test_to_string_decodes_from_its_byte_with_one_register_operand() {
    let op = Opcode::from_byte(0xD0).expect("TO_STRING is assigned byte 0xD0 (ADR-0043)");
    assert_eq!(op, Opcode::ToString);
    assert_eq!(op.mnemonic(), "TO_STRING");
    assert_eq!(op.operand_len(), 2, "TO_STRING takes dst(u8), src(u8)");
}

#[test]
fn test_all_opcodes_have_distinct_byte_values() {
    let mut bytes: Vec<u8> = Opcode::ALL.iter().map(|op| op.to_byte()).collect();
    bytes.sort_unstable();
    let count = bytes.len();
    bytes.dedup();
    assert_eq!(bytes.len(), count, "duplicate opcode byte value");
}

#[test]
fn test_all_declared_opcodes_decode_from_their_own_byte() {
    let undecodable: Vec<Opcode> = Opcode::ALL
        .iter()
        .copied()
        .filter(|op| Opcode::from_byte(op.to_byte()) != Some(*op))
        .collect();
    assert_eq!(undecodable, [], "these opcodes do not round-trip");
}

#[test]
fn test_mnemonic_matches_appendix_e_spelling() {
    assert_eq!(Opcode::AddI64.mnemonic(), "ADD_I64");
}

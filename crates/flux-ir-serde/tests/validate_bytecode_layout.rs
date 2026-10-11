//! FLUX-F46 regression: `validate_bytecode` must read the i32 jump
//! target at the offset the emitter wrote it to.
//!
//! * `JUMP` (I32 operand): op(1) + i32 target(4) → target at offset+1.
//! * `COND_JUMP(_NOT)` (REG_U32): op(1) + reg(1) + i32(4) → target at
//!   offset+2.

use flux_ir_serde::validate_bytecode;
use flux_syntax::opcode::Opcode;

/// `JUMP` at 0, real target i32 = 1 (relative to next instruction at
/// offset 5 → target offset 6). Program is 7 bytes: JUMP(5) + HALT(1) +
/// HALT(1).
#[test]
fn jump_target_still_read_at_offset_one() {
    let mut program = Vec::new();
    program.push(Opcode::Jump as u8);
    program.extend_from_slice(&1i32.to_le_bytes()); // target = 1
    program.push(Opcode::Halt as u8); // offset 5
    program.push(Opcode::Halt as u8); // offset 6 — the target
    validate_bytecode(&program).expect("JUMP with a valid target must validate");
}

/// `COND_JUMP_NOT` at 0, reg = 1, real target i32 = 1 (relative to next
/// instruction at offset 6 → target offset 7). Program is 8 bytes:
/// COND_JUMP_NOT(6) + HALT(1) + HALT(1).
///
/// Before the fix, `validate_bytecode` read the i32 at `offset+1` —
/// bytes `[reg=1, t0=1, t1=0, t2=0]` = 257, giving `target = 6 + 257 =
/// 263 > 8`, and rejected a valid program.
#[test]
fn cond_jump_targets_are_read_at_offset_two() {
    let mut program = Vec::new();
    program.push(Opcode::CondJumpNot as u8);
    program.push(0x01); // reg = 1
    program.extend_from_slice(&1i32.to_le_bytes()); // real target = 1
    program.push(Opcode::Halt as u8); // offset 6
    program.push(Opcode::Halt as u8); // offset 7 — the target
    validate_bytecode(&program).expect(
        "COND_JUMP_NOT with a valid target must validate once the i32 is read at +2",
    );
}

/// `COND_JUMP` (not `_NOT`) must be read the same way.
#[test]
fn cond_jump_opcode_has_the_same_layout() {
    let mut program = Vec::new();
    program.push(Opcode::CondJump as u8);
    program.push(0x02); // reg = 2
    program.extend_from_slice(&0i32.to_le_bytes()); // target = 0 → next offset (6)
    program.push(Opcode::Halt as u8); // offset 6 — the target
    validate_bytecode(&program).expect("COND_JUMP with a valid target must validate");
}

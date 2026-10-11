# FLUX-F46 — `validate_bytecode` reads `COND_JUMP(_NOT)` targets at the wrong offset

**Status:** OPEN (fix landed in same commit)
**Found:** round-90 bug hunt (`flux-ir-serde/wire` sweep)
**Severity:** medium (a malformed `COND_JUMP` whose real target is out
of range may still validate, and a well-formed one may be rejected
based on the byte immediately after the opcode — but the blast radius
is bounded: only the `wire` decoder's LANE-D validation path is
affected, and today no production caller exercises it)

## Summary

`crates/flux-ir-serde/src/wire/core.rs::validate_bytecode` (pass 2)
reads the jump target operand of every `JUMP` / `COND_JUMP(_NOT)` at
`decoded.offset + 1`:

    let jump = match opcode {
        Opcode::Jump => Some(read_i32(bytecode, decoded.offset + 1)),
        Opcode::CondJump | Opcode::CondJumpNot => Some(read_i32(bytecode, decoded.offset + 1)),
        _ => None,
    };

But the wire format — confirmed by the emitter
(`flux_ir/src/lower/bytecode.rs::jump_placeholder`), by the reference
VM's `Instruction::i32(offset)`, and by `_ = instruction.instruction_len`
in `decode.rs` — places the i32 operand at **different offsets** in the
two cases:

* `JUMP` (`I32` operand): `op(1) + i32 target(4)` → the i32 is at
  absolute `offset + 1`. The validator is correct here.
* `COND_JUMP` / `COND_JUMP_NOT` (`REG_U32` operand):
  `op(1) + reg(1) + i32 target(4)` → the i32 is at absolute
  `offset + 2`. **The validator reads at `offset + 1`**, i.e. it reads
  `[reg_byte, t0, t1, t2]` — the low byte of a value the format never
  intended to be a whole `i32` — where the real target is
  `[t0, t1, t2, t3]`.

## Reproduction

Hand-assemble a 6-byte `COND_JUMP_NOT reg=1 target=1` followed by two
`HALT`s (8 bytes total, target 7 = `len - 1`, a valid instruction
boundary):

* The real target is 7 → validation should accept.
* The validator reads `[1, 1, 0, 0]` as i32 = `0x00000101` = 257,
  computes `base (6) + 257 = 263 > 8`, and rejects with
  `MalformedBytecode { detail: "jump-out-of-range" }`.

Symmetrically, a malformed `COND_JUMP_NOT` whose real target is 263 but
whose byte pattern at `offset+1..offset+5` happens to encode a valid
target is **accepted**.

## Fix

Branch on the opcode's operand layout, matching the emitter:

    let jump = match opcode {
        Opcode::Jump => Some(read_i32(bytecode, decoded.offset + 1)),
        Opcode::CondJump | Opcode::CondJumpNot => {
            Some(read_i32(bytecode, decoded.offset + 2))
        }
        _ => None,
    };

`Opcode::operand_len()` already distinguishes the two (5 bytes vs 6);
the offset can be derived as `offset + operand_len - 4` if desired, so
the layout and the parser share one source of truth.

## Regression tests

`crates/flux-ir-serde/tests/validate_bytecode_layout.rs` (added in this
commit):
* `cond_jump_targets_are_read_at_offset_two` — the reproduced
  `COND_JUMP_NOT ...` above validates with the fix.
* `jump_target_still_read_at_offset_one` — pins that `JUMP` remains
  correct.

## Where it matters

Today only `crates/flux-ir-serde/tests/round_trip.rs` calls the
validator; the production decode path has not yet been switched to
use it (LANE-D is scaffolded). The bug is latent — a future switch to
call `validate_bytecode` on every decoded frame will surface it as
either false positives (real programs rejected) or false negatives
(malformed programs accepted). Fixing it now removes the trap.

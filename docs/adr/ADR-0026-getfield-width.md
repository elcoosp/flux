# ADR-0026: GET_FIELD bytecode width corrected to 4 bytes (REG_U16_REG)

**Status:** Accepted
**Date:** 2026-08-24
**Decision Drivers:** The golden ISA vectors (FLUX-002) and `flux-vm-ref` (FLUX-005) must
agree on every instruction's byte layout. During conformance testing, `GET_FIELD`
was found to be the single remaining divergence.

## Context

Appendix E §E.1 lists argument shapes but is internally inconsistent about
`GET_FIELD`. The original `flux-syntax` width table mapped `GET_FIELD` to
`REG_REG_U16` (3 operand bytes: `reg(u8), u16`). A 3-byte encoding cannot carry
all three of `dst`, `idx`, and `obj`, so the frozen golden vectors encoded
`GET_FIELD` as **4 bytes** (`dst(u8), idx(u16), obj(u8)` — the `REG_U16_REG`
shape shared by `SET_FIELD` and `EXTRACT_FIELD`).

This mismatch caused instruction-boundary misalignment: the VM decoded 3 bytes
per `GET_FIELD`, while the vectors supplied 4, shifting every subsequent
instruction and producing spurious `IndexOutOfBounds` faults.

## Decision

Adopt the 4-byte `REG_U16_REG` layout for `GET_FIELD`, matching `SET_FIELD` and
`EXTRACT_FIELD`, with operand order **`(dst, idx, obj)`**:

- `flux-syntax` `Opcode::operand_len(GetField)` → `width::REG_U16_REG` (was
  `REG_REG_U16`, now removed as dead code).
- `flux-vm-ref` `GetField` arm reads `dst = u8(0)`, `idx = u16(1)`, `obj = u8(3)`.
- The generator (`/tmp/gen_vectors.py`) encodes `GET_FIELD` as
  `enc(op, dst, idx, obj)` with layout `["u8","u16","u8"]`, consistent with the
  VM.

`REG_REG_U16` was deleted because no opcode uses a 3-byte `reg, u16` shape after
this change (kept the crate warning-free per AGENTS.md §1.2).

## Consequences

- All 71 golden vectors now decode and execute identically in `flux-vm-ref`.
- The Swift and Kotlin runtimes (FLUX-006/007) must implement `GET_FIELD` as
  4 bytes `(dst, idx, obj)` or they will fail the same conformance suite.
- `REG_U16_REG` is now the uniform shape for every record field-access opcode.

## References

- Appendix E §E.1 (width table) — source of the inconsistency.
- ADR-0022 (byte-length erratum) — same class of §E.1 defect.
- FLUX-002 (golden vectors), FLUX-005 (`flux-vm-ref`).

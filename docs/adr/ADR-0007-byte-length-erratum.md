# ADR-0007: Byte-length erratum in Appendix E §E.5

**Status:** Accepted
**Date:** 2026-08-24
**Decision Drivers:** Correctness of golden ISA vector `bytecode_hex` lengths (contract FLUX-002) and of the production VM instruction decoders (FLUX-005/006/007).

## Context and Problem Statement

Appendix E §E.5 presents the bytecode for `count = count + 1` and states: *"Total: 21 bytes. At 1 gas per instruction = 4 gas."*

The gas figure (4) is correct per ADR-0006 (HALT exempt; four semantic instructions). The **byte length (21) is wrong.** Decoding the instruction sequence against the §E.1 operand widths:

| Instruction | Encoding per §E.1 | Byte count |
|---|---|---|
| `READ_SIGNAL r0, 0x00000001` | opcode(1) + dst(1) + signal_id u32(4) | 6 |
| `LOAD_INT_CONST r1, 0x0000000000000001` | opcode(1) + dst(1) + value i64(8) | 10 |
| `ADD_I64 r0, r0, r1` | opcode(1) + dst(1) + a(1) + b(1) | 4 |
| `WRITE_SIGNAL 0x00000001, r0` | opcode(1) + signal_id u32(4) + src(1) | 6 |
| `HALT` | opcode(1) | 1 |
| **Total** | | **27** |

The normative byte count is **27**, not 21. The error appears to stem from dropping one register byte somewhere in the hand count (e.g. counting `READ_SIGNAL r0, id` as 5 bytes instead of 6).

## Considered Options

**Option A — Treat §E.5's "21 bytes" as authoritative and shorten encodings.**
- Pros: Matches the text.
- Cons: Contradicts the §E.1 operand-width table that the same appendix defines and that the three VMs must implement. Would force divergent, incorrect decoders.

**Option B — Treat the §E.1 width table as authoritative; the "21 bytes" note is an erratum.**
- Pros: Keeps a single consistent encoding (width table) that all decoders follow. Matches the actual byte layout shown in §E.5's annotated listing (`0x10 r0 0x00000001` etc., which sums to 27).
- Cons: Requires correcting the prose note (does not change any normative table).

## Decision Outcome

**Chosen: Option B.** The §E.1 operand-width table is normative; the "21 bytes" prose in §E.5 is an erratum and should be read as **27 bytes**. All ISA vectors and VM decoders compute instruction and frame lengths strictly from the §E.1 width table. The reference encoding (27 bytes) is reproduced authoritatively in the FLUX-002 README and in this ADR's references.

Reference (byte-exact, little-endian fields, as Appendix E.4 specifies):

```
10 00 00 00 00 01   READ_SIGNAL  r0, signal_id=1
b0 01 00 00 00 00 00 00 00 01   LOAD_INT_CONST r1, 1
20 00 00 01   ADD_I64 r0, r0, r1
11 00 00 00 01 00   WRITE_SIGNAL signal_id=1, r0
00   HALT
```

Hex (no spaces): `100000000001b0010000000000000001 200000 0110000000100 00` (concatenated: `100000000001b0010000000000000001200000 011000000010000` → `100000000001b0010000000000000001200000011000000010000`). See FLUX-002 README for the canonical string.

## Consequences

**Positive:** One unambiguous byte length; vectors and decoders agree.
**Negative:** Appendix E prose must be mentally corrected (or edited) when read.
**Neutral:** Does not affect gas accounting (ADR-0006).

## References
- Appendix E §E.1 (opcode table / operand widths), §E.4 (bytecode layout), §E.5 (example).
- ADR-0006 (gas accounting).
- Contract FLUX-002 / D6.

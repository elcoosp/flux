---
title: VM Instruction Set
description: The Flux host VM opcode reference (Appendix E) — signal ops, monomorphized arithmetic, control flow, and closures.
---

The host embeds a **register-based bytecode VM**. Instructions are monomorphized:
there is no generic `ADD` with tag dispatch — there are `ADD_I64`, `ADD_F64`, etc.
This keeps the hot path branch-free and the trace byte-identical across hosts.

> Opcodes and operand encodings below are normative and taken from Appendix E of
> the specification. Do not add opcodes without an ADR and a protocol version
> bump.

## Signal operations

| Opcode | Mnemonic | Args | Description |
|---|---|---|---|
| `0x10` | `READ_SIGNAL` | reg_dst(u8), signal_id(u32) | Read a signal into a register |
| `0x11` | `WRITE_SIGNAL` | signal_id(u32), reg_src(u8) | Write a register's value to a signal |

A handler closure ends by writing the signals the dispatch consumed. Those written
ids become the `signals` trace event (sorted ascending).

## Integer arithmetic (monomorphized)

| Opcode | Mnemonic | Args | Description |
|---|---|---|---|
| `0x20` | `ADD_I64` | dst, a, b (u8 each) | `dst = a + b` (i64) |
| `0x21` | `SUB_I64` | dst, a, b | `dst = a - b` |
| `0x22` | `MUL_I64` | dst, a, b | `dst = a * b` |
| `0x23` | `DIV_I64` | dst, a, b | `dst = a / b` |
| `0x24` | `MOD_I64` | dst, a, b | `dst = a % b` |

Floating-point variants (`ADD_F64`, …) exist in the same `0x2x` band with the
`F64` suffix. The counter's `count = count + 1` compiles to `READ_SIGNAL`,
`LOAD_INT_CONST`, `ADD_I64`, `WRITE_SIGNAL`.

## Control flow & closures

Handler and prop-thunk closures share the `ClosureRef` encoding (D.7): an 8-byte
BLAKE3 hash (content address), a bytecode offset/length into the shared blob, the
captured signal ids, and a source span. Phase 3 prop thunks run locally from the
dirty set — `r0` is reserved for node context, `r1` holds the `ALLOC_RECORD`
result on `HALT`, and `prop_layout` maps record fields to prop indices.

## Fault policy

A thunk or handler fault ⇒ the node keeps its prior props (render stale, never
blank); the error surfaces through the existing overlay path and a trace `error`
event is recorded. The view is never torn down.

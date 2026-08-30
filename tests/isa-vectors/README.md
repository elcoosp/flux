# Golden ISA Vectors — `/tests/isa-vectors/`

These JSON fixtures are the **behavioral source of truth** for the Flux VM
instruction set (Appendix E). All three VM implementations must pass the same
vectors:

- `flux-vm-ref` (Rust reference oracle, FLUX-005)
- `FluxBytecodeVM` Swift (FLUX-006)
- `FluxBytecodeVM` Kotlin (FLUX-007)

The vectors are **frozen after merge** (boundary contract R8). Corrections go
through the orchestrator, who re-runs all three VM conformance suites.

## Schema

```json
{
  "name": "add_i64_basic",
  "description": "ADD_I64 computes 41+1.",
  "bytecode_hex": "b0000000000129...",   // lowercase hex, NO spaces
  "initial_signals": [ { "id": 1, "value": { "type": "Int", "value": 41 } } ],
  "payload": null,                        // entry r0 value, or null
  "expected_signals": [ { "id": 1, "value": { "type": "Int", "value": 42 } } ],
  "expected_registers": { "r2": { "type": "Int", "value": 42 } },
  "expected_error": null,                 // or one of the error kinds
  "expected_gas_used": 4
}
```

### Value encodings (`type` field)

| `type`   | `value`            | Notes                                   |
|----------|--------------------|-----------------------------------------|
| `Int`    | JSON integer (i64) | two's-complement                         |
| `Float`  | JSON number (f64)  | `inf` / `-inf` / `nan` as JSON strings  |
| `Bool`   | JSON bool          |                                         |
| `Str`    | JSON integer       | string-table id                         |
| `List`   | JSON array of vals |                                         |
| `Record` | JSON array of vals | field order = insertion order           |
| `Null`   | (absent)           |                                         |

### Error kinds (`expected_error`)

`GasExhausted`, `MemoryExhausted`, `IndexOutOfBounds`, `NullDereference`,
`InvalidDispatch`, `TypeMismatch`, `DivByZero`.

> `DivByZero` is **not** listed in Appendix E §E.6 — see the spec gaps section
> below. It is included here as the agreed behavior for `DIV_I64`/`MOD_I64` by
> zero, pending an ADR/spec edit.

## Encoding rules

1. Bytecode is a flat byte sequence. Each instruction is `opcode_byte` followed
   by operand bytes whose count and shape come from Appendix E §E.1 ("Args
   (bytes)"). There is **no alignment padding**.
2. Multi-byte operands are **little-endian**: `u16`/`u32`/`i32`/`i64`/`f64`.
3. Opcode byte values come from `crates/flux-syntax/src/opcode/raw.rs` (or
   Appendix E §E.1). Do not invent values.
4. **Gas** (`expected_gas_used`) = the number of *executed, non-`HALT`*
   instructions (ADR-0021 — `docs/adr/ADR-0021-gas-accounting.md`). `HALT` (0x00)
   is free; `GAS_CHECK` (0xC0) is charged 1 gas before its budget check. Handler
   entry budget in `r15` is 100,000.
5. **Lengths** are derived strictly from the §E.1 width table (ADR-0022 —
   `docs/adr/ADR-0022-byte-length-erratum.md`); do not trust prose byte counts in
   the appendices. The canonical `count = count + 1`
   example is **27 bytes**, not the "21 bytes" stated in §E.5.
6. The instruction pointer is a **byte offset** into the bytecode (§E.4). Jump
   offsets (`JUMP`, `COND_JUMP`, `COND_JUMP_NOT`, `MATCH_TAG`) are relative to
   the *start of the next instruction* (i.e. `target = ip_after_instruction +
   offset`).

## Coverage matrix (all 54 opcodes, every opcode has ≥1 happy-path vector)

| Opcode  | Mnemonic        | Vector(s) |
|---------|-----------------|-----------|
| 0x00    | HALT           | halt_basic |
| 0x01    | NOP            | nop_basic |
| 0x10    | READ_SIGNAL    | read_signal_basic, signal_roundtrip |
| 0x11    | WRITE_SIGNAL   | write_signal_basic, signal_roundtrip |
| 0x20    | ADD_I64        | add_i64_basic, add_i64_overflow_min, add_i64_max |
| 0x21    | SUB_I64        | sub_i64_basic, sub_i64_min |
| 0x22    | MUL_I64        | mul_i64_basic, mul_i64_min |
| 0x23    | DIV_I64        | div_i64_basic, div_i64_by_zero |
| 0x24    | MOD_I64        | mod_i64_basic, mod_i64_by_zero |
| 0x25    | NEG_I64        | neg_i64_basic |
| 0x26    | EQ_I64         | eq_i64_basic |
| 0x27    | LT_I64         | lt_i64_basic |
| 0x28    | GT_I64         | gt_i64_basic |
| 0x29    | LTE_I64        | lte_i64_basic |
| 0x2A    | GTE_I64        | gte_i64_basic |
| 0x30    | ADD_F64        | add_f64_basic, add_f64_inf, add_f64_neginf |
| 0x31    | SUB_F64        | sub_f64_basic |
| 0x32    | MUL_F64        | mul_f64_basic |
| 0x33    | DIV_F64        | div_f64_basic, div_f64_by_zero |
| 0x34    | NEG_F64        | neg_f64_basic |
| 0x35    | EQ_F64         | eq_f64_basic, eq_f64_nan |
| 0x36    | LT_F64         | lt_f64_basic |
| 0x37    | GT_F64         | gt_f64_basic |
| 0x38    | I64_TO_F64     | i64_to_f64 |
| 0x39    | F64_TO_I64     | f64_to_i64 |
| 0x40    | AND_BOOL       | and_bool |
| 0x41    | OR_BOOL        | or_bool |
| 0x42    | NOT_BOOL       | not_bool |
| 0x50    | STR_CONCAT     | str_concat_basic |
| 0x51    | STR_INTERN     | str_intern_basic |
| 0x52    | STR_EQ         | str_eq_true |
| 0x53    | STR_LEN        | str_len_basic |
| 0x60    | JUMP           | jump_forward |
| 0x61    | COND_JUMP      | cond_jump_taken, cond_jump_not_taken |
| 0x62    | COND_JUMP_NOT  | cond_jump_not_taken2 |
| 0x70    | ALLOC_RECORD   | alloc_record_get_set |
| 0x71    | GET_FIELD      | alloc_record_get_set, get_field_null_deref, get_field_oob |
| 0x72    | SET_FIELD      | alloc_record_get_set |
| 0x73    | RECORD_EQ      | record_eq_true |
| 0x80    | ALLOC_LIST     | alloc_list_push_get |
| 0x81    | LIST_PUSH      | alloc_list_push_get |
| 0x82    | LIST_GET       | alloc_list_push_get, list_get_oob |
| 0x83    | LIST_LEN       | list_len_basic |
| 0x84    | LIST_CONCAT    | list_concat_basic |
| 0x90    | CALL_CAP       | call_cap_basic |
| 0xA0    | MATCH_TAG      | match_tag_taken, match_tag_not_taken |
| 0xA1    | EXTRACT_FIELD  | extract_field_basic |
| 0xB0    | LOAD_INT_CONST | load_int_const, (used throughout) |
| 0xB1    | LOAD_FLOAT_CONST | load_float_const |
| 0xB2    | LOAD_BOOL_CONST | load_bool_const |
| 0xB3    | LOAD_STR_CONST | load_str_const |
| 0xB4    | LOAD_NULL      | load_null |
| 0xB5    | MOV            | mov_basic |
| 0xC0    | GAS_CHECK      | gas_check_ok, gas_check_exhausted |

### Error / boundary / convention vectors (beyond per-opcode happy paths)

- `div_i64_by_zero`, `mod_i64_by_zero`, `add_i64_type_mismatch`, `div_f64_by_zero`
  (→ +inf), `add_f64_inf`, `add_f64_neginf`, `eq_f64_nan`
- `get_field_null_deref` (`NullDereference`), `get_field_oob`,
  `list_get_oob` (`IndexOutOfBounds`)
- `set_field_null_deref` (`NullDereference`), `set_field_non_record_type_mismatch`,
  `set_field_oob` (`IndexOutOfBounds`) — SET_FIELD now has symmetric null/type/bounds
  error paths to GET_FIELD (FLUX-086).
- `sub_i64_type_mismatch`, `mul_i64_type_mismatch`, `div_i64_type_mismatch`,
  `mod_i64_type_mismatch`, `eq_i64_type_mismatch` (Int operand against a Float
  operand → `TypeMismatch` per ADR-0024 monomorphized-op alignment), and
  `get_field_non_record_type_mismatch` (GET_FIELD on a non-record, non-Null value →
  `TypeMismatch`).
- `invalid_dispatch_bad_opcode` (unknown opcode byte `0xff` → `InvalidDispatch` at
  offset 0).
- `gas_check_exhausted` (`GasExhausted` via `GAS_CHECK`), `gas_exhausted_loop`
  (`GasExhausted` via infinite `JUMP` loop, 100,000 instructions)
- `call_cap_basic` (capability callback; stubbed host contract below)
- `reg_r0_payload` (r0 = entry payload), `reg_r15_gas_decrements` (r15 decrements)
- `signal_roundtrip` (READ→WRITE round-trip, second signal/type)
- `record_eq_true`, `list_concat_basic`

Total: **81 vectors** (71 prior + 10 error-path vectors added by FLUX-086).

> **Async cancellation (`AWAIT` + drop/cancel) is intentionally NOT a golden
> vector.** `AWAIT` is a v2 opcode and the v1 [`run`] entry point rejects it
> (`InvalidDispatch`), so resumable/cancel behaviour cannot be expressed in the
> v1 vector schema (which asserts a single terminal `expected_error` / final
> state). Cancellation parity is instead pinned in each runtime's dedicated async
> test (`AsyncSuspendResumeTests.swift` on iOS, `FluxBytecodeVmTest` on Android, and
> the `await_resume` test in `flux-vm-ref`), which assert the post-cancel signal
> graph matches the Rust oracle's `SuspendState`: the `Pending` result cell is left
> untouched and no signal was written before the `AWAIT`. The oracle's
> `run_resumable`/`resume` is the source of truth for that contract.

## Host contract assumed for CALL_CAP

The oracle's host is stubbed. `CALL_CAP result_reg, cap_id, method_id, args_reg`
with `cap_id=1, method_id=1` is interpreted as: read `args_reg` (a `Record`), take
its field 0, write that value into signal 99, and also leave it in `result_reg`.
Any other `cap_id`/`method_id` raises `TypeMismatch`. Production hosts define
real capabilities per `capabilities.flux`; this stub exists only so the vector is
decodable and deterministic.

## Spec gaps found while authoring (flagged for ADR/spec edit)

1. **`DivByZero` is not an enumerated error in Appendix E §E.6**, yet integer
   division by zero must fail. Vectors encode it as `DivByZero`. → see
   `docs/adr/ADR-0023-div-by-zero-error.md`.
2. **`GET_FIELD` on `Null`**: §E.6 lists `NullDereference` for "GET_FIELD on
   Null value," but `GET_FIELD` on a *non-record, non-null* value (e.g. an
   `Int`) is a type error. Vectors encode `Null` → `NullDereference` and other
   non-records → `TypeMismatch`. → see `docs/adr/ADR-0024-getfield-null.md`.
3. **`MemoryExhausted` is not asserted as a predicted value** here: the 16 MB
   pool threshold depends on the oracle's allocation accounting, which is not
   pinned by the spec. Vectors exercise allocations and assert *success*; the
   threshold is left to the oracle. No false prediction is committed.
4. **Float `DIV` by zero** follows IEEE 754 (`x/0.0 = ±inf`), not an error —
   distinct from integer `DIV` by zero (which errors). Vectors reflect this.

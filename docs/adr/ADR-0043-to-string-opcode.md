# ADR-0043 — `TO_STRING` opcode for prop-thunk string interpolation

- Status: Accepted
- Date: 2026-08-26
- Scope: `flux-syntax` (opcode), `flux-vm-ref` (oracle), `flux-ir` (lowering),
  `flux-devserver` (frame shipping), iOS `FluxBytecodeVM` (FLUX-006),
  Android `FluxBytecodeVM` (FLUX-007)
- Supersedes: none
- Superseded by: none
- Related: ADR-0027 (prop thunks / signal metadata, Phase 2/3), ADR-0028
  (host string interning)

## Context

In dev mode, dynamic prop expressions — string interpolations such as
`"tapped ${count} times"` — collapsed to a static placeholder (`"{…}"`) on the
wire, because the lowering emitted a single `LOAD_STR_CONST` of the placeholder
text and never evaluated `count` against the live signal graph. ADR-0027 Phase
3 introduces **prop thunks**: a closure that evaluates every prop of a node and
leaves an `ALLOC_RECORD` of values in `r1` at `HALT`, which the host runs on
dirty reconciliation to materialise props locally.

A prop thunk must build an interpolated string at runtime. The VM therefore
needs an opcode that converts any value to its textual form so that
`STR_CONCAT` can splice the pieces into a single `Str` register. No such opcode
existed; without it the thunk cannot produce a string prop from a signal read.

## Decision

Add a single new opcode `TO_STRING` (byte `0xD0`):

- Operands: `dst(u8)`, `src(u8)` — width `REG` (2 operand bytes), matching the
  existing `MOV`/`STR_LEN` shape.
- Semantics: `dst := Str(intern(render(src)))`, where `render` is the
  cross-runtime textual contract below and `intern` binds the text to a fresh
  `StringId`.

`render` is a **cross-runtime contract** shared by the Rust oracle
(`flux-vm-ref`), the Swift runtime and the Kotlin runtime, so a node's
materialised props are byte-identical across all three and comparable to the
release codegen output in the parity suite:

| Value            | Rendered text                                  |
|------------------|------------------------------------------------|
| `Int(i)`         | `i.to_string()`                                |
| `Float(f)` finite, integral | `String(format: "%.1f", f)` (`1.0`, not `1`) |
| `Float(f)` other | `f.to_string()` (IEEE)                         |
| `Bool(b)`        | `"true"` / `"false"`                           |
| `Str(id)`        | resolve `id` through the live string table     |
| `HandlerRef(id)` | `"handler(<id>)"`                              |
| `List`/`Record`  | `[a, b]` / `{idx: v, …}` (recursive)           |
| `Null`           | `"null"`                                       |

The oracle (`flux-vm-ref`) owns **no** live string table, so it interns the
rendered text into the reserved high half (`0x8000_0000 | fnv1a(text)`) via a
deterministic synthetic id. The golden ISA vector `to_string_int` pins this
behaviour (`42 → StringId 2279835011`), keeping the oracle self-consistent for
the conformance suite even though its `Str` output is not the host's real
interned id.

### Lowering changes (ADR-0027 T14 / Phase 3)

`compile_value` for `ExprKind::Str(parts)` now walks each part:

- `StrPart::Text(t)` → `LOAD_STR_CONST` (as before).
- `StrPart::Interp(e)` → compile `e` to a register, `TO_STRING` it, then
  `STR_CONCAT` into the running result.

A literal with no interpolation still collapses to a single `LOAD_STR_CONST`
(byte-for-byte the previous behaviour), so static props pay no cost. The
compiled thunk's bytecode is shipped to the host inside the frame's shared
handler blob: `emit_signal_metadata` already recorded the thunk as a
`ClosureIR`, and the dev-server pipeline now folds `lowered.prop_thunks` into the
closure list it hands to `Frame::init`/`build_delta`, so the host can resolve a
node's `prop_thunk` `ClosureRef` to real bytecode by hash.

## Consequences

- String interpolation in dev mode now evaluates against the live signal graph
  instead of collapsing to `"{…}"`.
- One new opcode (`0xD0`); `Opcode::ALL` grows from 54 → 55. Appendix E §E.1 and
  the count assertion in `crates/flux-syntax/tests/opcodes.rs` are updated
  together.
- The three runtimes agree on `render` text, enforced by the shared golden ISA
  vector convention; a fourth runtime would need the same rendering to stay in
  parity.
- Dynamic interning at runtime stays within the existing `StringResolver`
  abstraction on each host (the ADR-0028 proxy range), so no wire-format change
  was required beyond what ADR-0027 already added.

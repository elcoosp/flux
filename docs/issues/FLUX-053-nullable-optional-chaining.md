---
id: FLUX-053
status: done
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - PRD-L
labels:
  - language
  - types
  - adr
source: CHANGELOG.md §PRD-S (deferred: "nullable/optional chaining ergonomics")
related_adrs:
  - ADR-0035
  - ADR-0051
  - ADR-0054
---

# Closure

Language surface (`?.` → `ExprKind::OptField`, `Null` literal) was landed under
ADR-0051. This issue closes the bytecode half: a new wire opcode `IS_NULL`
(`0xD1`, `REG_REG`) plus the `Null` literal and the `OptField` value-form
desugar in `flux-ir/src/lower/bytecode.rs` (lowers `base?.field` to
`IS_NULL`/`COND_JUMP_NOT`/`LOAD_NULL`/`GET_FIELD`). The Rust VM (`flux-vm-ref`)
implements `IS_NULL`; conformance vectors `is_null_true`/`is_null_false` and the
`flux-ir` `opt_field_lowers_to_is_null_and_get_field` test cover it (suite
green: 223 tests). Swift/Kotlin VMs must mirror `0xD1` before on-device
handlers emit `?."` — tracked as a follow-up, does not block the Rust side
(parity runs through `flux-vm-ref`).


# FLUX-053: Nullable / optional chaining ergonomics (ADR-gated)

- **Lane:** LANE-L (Phase 1)
- **Depends on:** PRD-L
- **Source:** `CHANGELOG.md` §PRD-S deferred
- **Related ADRs:** ADR-0035 template

## Problem Statement

Real apps need nullable values + safe chaining (`?.`). PRD-S deferred it as
ADR-gated (a type-system gap for real apps, roadmap §3).

## Solution

ADR for the nullability model, then extend `flux-types` (a `Null`/`Option` value
already exists in the VM (`FluxValue::Null`)) + the `?.` lowering, then parity
tests.

## Implementation Decisions

- Reuses `FluxValue::Null` so the runtime contract is already there; this is the
  language/type surface.
- Targets the frozen grammar.

## Testing Decisions

- A `?.` chain over a `Null` value lowers + type-checks; parity trace pins dev/release.

## Out of Scope

- Structural-vs-nominal prop typing (FLUX-054).

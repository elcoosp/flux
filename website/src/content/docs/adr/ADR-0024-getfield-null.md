---
title: "ADR-0009: GET_FIELD error discrimination (Null vs non-record)"
---

# ADR-0009: GET_FIELD error discrimination (Null vs non-record)

**Status:** Accepted
**Date:** 2026-08-24
**Decision Drivers:** Correctness of the golden ISA vectors (FLUX-002) and of the
three VM implementations (FLUX-005/006/007) for `GET_FIELD`.

## Context and Problem Statement

Appendix E §E.6 lists `NullDereference` with the trigger "GET_FIELD on Null value."
But `GET_FIELD` can also be applied to a value that is neither `Null` nor a
`Record` (e.g. an `Int`). The §E.6 table does not enumerate what error that is,
and §E.6 separately lists `TypeMismatch` for "ADD_I64 on non-Int value."

Two reasonable readings:
1. Every non-`Record` operand (including `Null`) → `TypeMismatch`.
2. `Null` operand → `NullDereference`; other non-`Record` operands → `TypeMismatch`.

## Considered Options

**Option A — uniform `TypeMismatch` for all non-records.**
- Pros: Simple; one rule.
- Cons: Loses the specific, actionable `NullDereference` the spec explicitly
  promises for the `Null` case. A null deref and a type error are different
  debugging signals.

**Option B — discriminate: `Null` → `NullDereference`; other non-records →
`TypeMismatch`.**
- Pros: Honors the spec's explicit `NullDereference` trigger; keeps `TypeMismatch`
  for genuine type errors. Most informative diagnostics.
- Cons: Slightly more branching in the decoder (a null check before the
  record-type check).

## Decision Outcome

**Chosen: Option B.** `GET_FIELD` on `Null` raises `NullDereference`; `GET_FIELD`
on any other non-`Record` value raises `TypeMismatch`. `SET_FIELD` follows the
same discipline for consistency (though the spec only names `GET_FIELD` for
`NullDereference`; `SET_FIELD` on `Null` also raises `NullDereference` by
analogy, and on other non-records `TypeMismatch`).

The vectors encode this: `get_field_null_deref.json` asserts `expected_error:
"NullDereference"`; a `GET_FIELD` on, say, an `Int` would assert `TypeMismatch`.

## Consequences

**Positive:** Matches the explicit spec trigger; precise diagnostics; all three
VMs agree.
**Negative:** §E.6 does not spell out the non-record-non-null case; this ADR fills
that gap. If the spec is later edited, it should state both cases explicitly.

## References
- Appendix E §E.6 (error conditions).
- FLUX-002 vectors: get_field_null_deref, get_field_oob.
- ADR-0008 (sibling error-kind clarification for the same VM).

# ADR-0008: DivByZero as an explicit VM error kind

**Status:** Accepted
**Date:** 2026-08-24
**Decision Drivers:** Correctness of the golden ISA vectors (FLUX-002) and of the
three VM implementations (FLUX-005/006/007) for integer division by zero.

## Context and Problem Statement

Appendix E §E.6 enumerates the VM error kinds: `GasExhausted`,
`MemoryExhausted`, `IndexOutOfBounds`, `NullDereference`, `InvalidDispatch`,
`TypeMismatch`. Integer `DIV_I64`/`MOD_I64` by zero is not covered. Rust's `i64`
division by zero panics, so the VM must define a defined failure rather than
propagate a panic (which would crash the host app — violating the ADR-0002
"never crash on user code" property).

## Considered Options

**Option A — Reuse `TypeMismatch`.** Treat `x/0` as a type error.
- Pros: No new error kind; matches "should never happen (monomorphization
  guarantees types)" framing in §E.6.
- Cons: It is NOT a type error — both operands are well-typed `Int`. Misleading
  diagnostic; conflates two distinct failure modes.

**Option B — Add `DivByZero` as a distinct error kind.**
- Pros: Precise, actionable diagnostic ("division by zero at span"). Matches
  every other production VM (JVM `ArithmeticException`, Swift `FloatingPoint`
  trap, Kotlin `ArithmeticException`).
- Cons: Extends the §E.6 table (a one-line spec edit).

**Option C — Mirror IEEE-754 into integers** (wrap/inf).
- Cons: Integers have no infinity; wrapping `x/0` is silent data corruption.
  Rejected.

## Decision Outcome

**Chosen: Option B.** `DIV_I64`/`MOD_I64` by a zero divisor raise `DivByZero`,
carrying the current `Span`. Floating-point `DIV_F64` by zero remains IEEE-754
(`±inf`), per ADR-consistent numeric semantics — it is **not** an error.

The vectors in `/tests/isa-vectors/` encode this: `div_i64_by_zero.json` and
`mod_i64_by_zero.json` assert `expected_error: "DivByZero"`; `div_f64_by_zero.json`
asserts `expected_registers` `r2 = +inf`.

## Consequences

**Positive:** Precise diagnostics; all three VMs share one rule; no host crash.
**Negative:** §E.6 needs a one-line addition (`DivByZero` between
`NullDereference` and `InvalidDispatch`). This ADR is the normative interim
statement until the spec is edited.
**Neutral:** Float division by zero unchanged (IEEE inf).

## References
- Appendix E §E.6 (error conditions), §E.1 (opcode table).
- ADR-0002 (host-authoritative state, no crashes on user code).
- FLUX-002 vectors: div_i64_by_zero, mod_i64_by_zero, div_f64_by_zero.

# ADR-0051: Nullable / optional-chaining (`?.`) ergonomics

- **Status:** Accepted
- **Date:** 2026-08-29
- **Supersedes / related:** PRD-S (deferred nullable/optional chaining),
  FLUX-053, ADR-0035 (parser grammar extensions), ADR-0044 (first-class async)

## Context

Real apps need nullable values plus safe chaining (`?.`). The runtime contract
is already present — `FluxValue::Null` exists in `flux-vm-ref` and the `Option[T]`
type is a fully-supported `TcType::Option` in `flux-types` (unify, formatting,
`from_str("Option")`, `TypeKind::Option`). What was missing is the **operator
surface**: there was no `?.` token, no AST node, and no type-checker rule.
FLUX-053's test decision — "`?.` chain over a `Null` value lowers +
type-checks; parity trace pins dev/release" — could not be met because the
token/AST/checker did not exist.

This ADR records the decided design and splits it into a landed slice (parser +
AST + type checker, verified by `flux-types`/`flux-parser` tests) and a
follow-up (handler-bytecode emission of value-form `If`, which `compile_value`
does not yet support — see Consequences).

## Decision

Adopt a single postfix `?.` operator (optional member access):

```flux
user?.profile?.name   // type: Option[String] when user: Option[User]
```

- **Grammar (ADR-0029, indentation-delimited):** `?.` is a new two-char token
  `TokenKind::QuestionDot`, lexed by the `flux-parser` lexer (which owns
  `TokenKind` — no `flux-syntax` edit, so the boundary contract is respected).
  It binds in `postfix_expr` exactly like `.`, producing a new
  `ExprKind::OptField { base, field }`.
- **Type checking:** given `base : Option[T]`, `base?.field` type-checks the
  `field` access against `T` and yields `Option[field_type]`. On a
  non-`Option` (concrete non-nullable) base it is a **type error** ("`?.`
  requires an Option base"). This mirrors the existing `Field` rule (record
  field lookup → inner type) but widens the result to `Option` because the
  chain short-circuits to `Null` when `base` is `Null`.
- **No new VM opcode.** Optional chaining is purely a source/type ergonomic; it
  desugars to the existing `If` + `Field` nodes at the IR level. The desugar is
  specified here and lands with the bytecode follow-up:

  ```
  base?.field   =>   if (base == Null) then Null else (base.field)
  ```

  i.e. an `ExprKind::If` whose then-branch is the `Null` placeholder and whose
  else-branch is `ExprKind::Field { base, field }`. The shadow-tree lower path
  already emits `NodeKind::If` and `Field`→`GET_FIELD`, so the **view** level is
  covered by this desugar unchanged.
- **Nullability propagation:** chaining composes — `a?.b?.c` yields
  `Option[...]` and short-circuits at the first `Null`.

## Consequences

- Nullable data (profiles, optional config, async results) is navigable without
  manual `if (x != Null)` boilerplate, and the type system still tracks whether
  a value may be absent (the result is `Option[...]`, forcing the caller to
  handle absence).
- The frozen grammar stays frozen in spirit: `?.` is a single new punctuation
  token added by the parser's own lexer, not a `flux-syntax` change.
- **Follow-up (not in this landing):** `compile_value` in
  `crates/flux-ir/src/lower/bytecode.rs` does not yet emit value-form `If`
  (it returns `unsupported handler operand` for `ExprKind::If` used as a value).
  Landing `a?.b` inside a handler body (e.g. an `onClick` that reads
  `user?.name`) requires: a `Null` literal expression, value-form `If`
  emission (`COND_JUMP_NOT` + result-register selection), and `LOAD_NULL`.
  These are tracked as the FLUX-053 bytecode follow-up; the parser/AST/checker
  slice and this ADR close the language-surface half of FLUX-053.

## Verification

- `flux-parser` round-trips `user?.profile?.name` (new lexer token +
  `OptField` AST).
- `flux-types` `type_check` accepts `base: Option[User]; base?.name` and yields
  `Option[String]`; rejects `base: User; base?.name` as a type error.
- ADR-0051 is the paper trail the issue's "ADR → production → close" flow
  required; the production (parser/AST/checker) is verified by the above tests.

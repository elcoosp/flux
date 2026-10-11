# FLUX-F27 — `_` in a `match` over an ADT is rejected by the type checker

**Status:** OPEN (fix landed in same commit)
**Found:** round-86 bug hunt (exhaustiveness probes)
**Severity:** medium (the spec's documented catch-all form
`match shape { Circle(r) => …, _ => 0 }` is rejected with a
misleading "variant `_` does not belong to `Color`" error; every Flux
program that uses `_` as a fallback over an ADT cannot type-check)

## Summary

The Flux parser accepts two wildcard spellings in a `match` arm:

* `*` (TokenKind::Star) → `MatchPatternKind::Wildcard`;
* `_` → the parser's identifier branch reads `_` as an `Ident` and, no
  `(`-list following, produces `MatchPatternKind::Variant { name: "_",
  fields: [] }`.

The `*` form is not documented in the Flux surface grammar — every
example and the spec §18.5 use `_`. So in practice every "catch-all"
arm reaches the type checker as `Variant { name: "_" }`.

`crates/flux-types/src/exhaust.rs::has_catch_all` correctly recognizes
this spelling (`MatchPatternKind::Variant { name, .. } => name.name ==
"_"`), so the exhaustiveness pass accepts the arm. But
`crates/flux-types/src/checker/scope.rs::bind_pattern_ty` (the
field-binding pass) does **not**: it looks up `_` in the ADT's variant
list and fails with

    variant `_` does not belong to `Color`

before the exhaustiveness pass is reached. Every `match` over an ADT
whose arms include a `_` fallback fails to type-check.

## Reproduction

    type Color =
      | RGB(Int)
      | HSL(Int)

    fn f(c: Color) -> Int {
      match c {
        RGB(r) => r
        _ => 0
      }
    }

`flux-types::type_check` fails with "variant `_` does not belong to
`Color`".

The same program with the trailing `*` spelling (`* => 0`) does parse as
`Wildcard` and type-checks — the two spellings must behave identically,
and the documented one (`_`) is the one that breaks.

## Impact

Any Flux program using the documented catch-all form in a `match` over
an ADT cannot type-check — including the `b33_adt` spec example if it
gains a wildcard arm, and every user program doing
`match color { RGB(r) => …, _ => … }`.

## Fix

`crates/flux-types/src/checker/scope.rs::bind_pattern_ty` and
`crates/flux-codegen-swift/src/backend_impl.rs::emit_match` and
`crates/flux-codegen-kotlin/src/backend_impl.rs::emit_match` all now
treat `MatchPatternKind::Variant { name, .. }` with `name.name == "_"`
as a wildcard:

* the checker returns `Ok(())` without a variant lookup (no binding);
* the Swift emitter writes `default:`;
* the Kotlin emitter writes `else ->` (the `emit_match_with_guards`
  variant already had this branch, added in F20).

This mirrors `has_catch_all` in `exhaust.rs`, which already applied the
same rule.

## Regression tests

* `crates/flux-parser/tests/…` (parser produces `Variant { name: "_" }`)
  — existing.
* `crates/flux-codegen-swift/src/backend_impl.rs` — new test that
  emits a `_` catch-all as `default:`.
* `crates/flux-codegen-kotlin/src/backend_impl.rs` — new test that emits
  a `_` catch-all as `else ->`.
* `crates/flux-parity/tests/parity.rs` — new test that the `match` over
  an ADT with a `_` fallback compiles end-to-end (verified at the
  pipeline level, not just parser).

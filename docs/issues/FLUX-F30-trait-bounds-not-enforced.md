# FLUX-F30 — Generic trait bounds on components are parsed but never enforced

**Status:** OPEN (documented; not fixed this round)
**Found:** round-86 bug hunt (type-checker probes)
**Severity:** medium (a component declaring `compo Counter[T: Foo]` where
`Foo` is not even a declared trait type-checks fine — the bound is parsed
into the AST but no pass ever consults it)

## Summary

`flux-parser` records a component's generic-parameter bounds faithfully
(`TypeParam { name, bound: Option<Ident>, .. }`, `ast/types.rs:72`). The
type checker stores each generic parameter in `Checker::generics` as a
bare `TcType::Var(PARAM_BASE + i)` and never reads `TypeParam::bound`.

Consequences:

1. **Undeclared trait.** `compo Counter[T: Foo]` where `Foo` is not a
   declared trait anywhere in the file still type-checks — the name
   `Foo` is never looked up.
2. **Unsatisfied bound at instantiation.** `Counter[Bool]()` where
   `Counter[T: Numeric]` and the component body calls `Numeric.zero()`
   never errors — the actual `Numeric` requirement is only discovered
   inside the state initializer, and the analysis only ever sees the
   `Constrained(id, ["Numeric"])` fresh variable it invents for
   `Numeric.zero()`.
3. **Multiple bounds.** `[T: A, T: B]` parses cleanly (the grammar
   accepts `,`-separated entries in `generic_params`), but only the
   first `bound` survives onto the `TypeParam`. The rest are silently
   dropped.

The net effect: a class of type errors the spec intends to catch
(§18.2, "generic components declare trait bounds; a call must satisfy
them") is silently accepted.

## Reproduction

    compo Counter[T: Foo]
      state n: Int = 0
      Text("x")

`flux-types::type_check` returns `Ok(_)`. It should error with
"unknown trait `Foo`".

    trait Numeric[T] { fn zero() -> T }

    compo Counter[T: Numeric]
      state count: T = Numeric.zero()
      Text("x")

    compo App
      Counter[Bool]()

Also `Ok(_)`. It should error at the call site: `Bool` does not
satisfy `Numeric`.

## What a fix needs

1. **Register bounds.** When `Checker` walks a component's generic
   parameters, for each `TypeParam { name, bound: Some(b), .. }`:
   * look up `b.name` in the environment; error with "unknown trait
     `Foo`" if it is not a `Binding::Trait`;
   * store the parameter as `TcType::Constrained(id, vec![b.name])` in
     `Checker::generics` instead of a bare `Var`.

2. **Check at instantiation.** `apply_callee` (which handles
   `Counter[Bool]()`) must walk the `generic_args` and, for each
   parameter with a bound, call `check_trait_bound(bound, arg_ty,
   span)` and surface `TraitError::NotSatisfied` as a `TypeError`.

3. **Multiple bounds.** Either extend `TypeParam` to `bounds:
   Vec<Ident>` (touching the parser and every consumer), or document
   that only one bound per parameter is supported and reject `,` in
   `generic_params`.

## Regression tests

Filed at `crates/flux-parity/tests/parity.rs`:
* `undeclared_trait_bound_is_a_known_gap` — `#[ignore]`d, asserts the
  current "silently OK" behavior for `[T: Foo]`.
* `bound_violated_at_instantiation_is_a_known_gap` — `#[ignore]`d,
  asserts the current "silently OK" behavior for `Counter[Bool]()`.

When the fix lands, un-ignore both and flip the assertions.

# FLUX-F68 — Generic top-level `fn` is used monomorphically across call sites

**Status:** OPEN (documented; not fixed — the fix is a change to how
`collect_adts` registers a generic function's type)
**Found:** round-114 bug hunt (type-checker probes)
**Severity:** medium (any top-level generic function — a first-class
Flux feature documented in the ADR-0017 surface grammar — can be used
at only ONE concrete type per compilation unit. A second use at a
different type fails with a misleading error naming the *first*
use's type.)

## Summary

`fn id[T](x: T) -> T { x }` is registered in the environment by
`checker/generics.rs::collect_adts`:

    // A top-level `fn`'s name is registered with the type
    // `Fn(param_tys, ret_ty)` where every occurrence of a generic
    // parameter has been rewritten to a fresh `Var(supply.fresh())`.
    let gen_vars: HashMap<String, TcType> = fn_decl
        .generics
        .iter()
        .map(|g| (g.name.name.clone(), TcType::Var(supply.fresh())))
        .collect();
    // … rewrite every param/ret through `gen_vars` …
    env.insert(
        fn_decl.name.text.clone(),
        Binding::Mono(TcType::Fn(param_tys, Box::new(ret_ty))),
    );

The binding is `Binding::Mono`, not `Binding::Poly`. So the SAME
`Var(id)` instance is returned by every lookup of `id`, and the first
call site's `expect` unifies it (`Var(id) := Int` for `id(1)`); every
subsequent call site finds the same variable already bound and fails.

`Binding::Poly` exists for exactly this purpose — the scheme is the
"generalised" type, and `instantiate` gives a fresh `Var` per lookup.
But the fn is registered `Mono`.

Contrast with `let`-bound values (`scope.rs::bind_let`), which use
`Binding::Poly(generalise(…))` so they can be used polymorphically.
Top-level functions are the one binding shape that skipped this step.

## Reproduction

    fn id[T](x: T) -> T { x }

    compo C
      state a: Int = id(1)
      state b: String = id("hi")     // <- fails
      Text("x")

`flux_types::type_check` fails with:

    expected `Int`, got `String`

The order of the uses does not matter: whichever call site is checked
first pins the type variable. The second one fails.

Using `id` at ONE type works:

    state a: Int = id(1)             // OK
    state a: String = id("hi")       // OK in isolation

## Impact

Any top-level generic function — `id[T]`, `map[T, U]`, a user-defined
`compose[A, B, C]` — is monomorphic. Users writing the same function
twice to work around this get a code-size and correctness penalty, and
the error message names the wrong call site as the problem.

The B.3 examples that use generics all use *generic components*, not
generic *functions*, so the parity snapshots do not exercise this path.

## What a fix needs

Change the binding to `Binding::Poly(generalise(&fn_ty, &env_free))`
at `collect_adts::Decl::Fn`. The scheme's `vars` are the variables
already drawn from `supply.fresh()` during the `gen_vars` rewrite; the
`env_free` set should be empty at module scope, so `generalise` picks
up every `Var` in the fn signature.

Then `lookup_value`'s `Binding::Poly(scheme) => instantiate(...)`
gives each call site a fresh copy of the fn type, and the second
`id("hi")` call succeeds.

The remaining subtlety is the interaction with the *body* check that
runs later in `check_decl`: the body must be checked against the
generalised scheme's parameters (each `Var` treated as rigid), not
against a concrete instantiation. That is what the module's doc
comment already says is intended:

> Generic params become fresh variables; the body is checked
> later in `check_decl`, which re-binds them in a fresh scope.

So the fix is a two-line change: register the fn as `Poly` in
`collect_adts`, and verify `check_decl` re-binds the params before
checking the body (it already does).

## Regression test

`crates/flux-parity/tests/parity.rs::generic_fn_two_uses_is_a_known_gap`
— added in this commit. Asserts the current failure
(`expected Int, got String`). When the fix lands, un-ignore and assert
successful compilation.

## Related

* `docs/issues/FLUX-F52-generic-adt-pattern-fields-not-substituted.md`
  — a generic-ADT instantiation gap.
* `docs/issues/FLUX-F30-trait-bounds-not-enforced.md` — trait-bound
  enforcement, which affects generic fns too.

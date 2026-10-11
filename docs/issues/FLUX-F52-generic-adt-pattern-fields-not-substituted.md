# FLUX-F52 — Pattern bindings of generic-ADT fields do not substitute the concrete instantiation

**Status:** OPEN (documented; not fixed — the fix is design-y but bounded)
**Found:** round-96 bug hunt (`flux-types/checker/generics.rs` +
`scope.rs`)
**Severity:** medium (a generic ADT with a concrete instantiation can
be constructed — `Box[Int]` — but cannot be pattern-matched without
the bound field's type being the generic parameter `T`. Any function
that consumes a `Box[Int]` and reads its payload fails to type-check
with `expected Int, got T`.)

## Summary

The type checker stores each ADT variant's declared field types on
`AdtDef.variants[i].fields`, with generic parameters left as
`TcType::Named("T", [])` (or as `Var(PARAM_BASE + i)` after the
component-prop rewrite — see `checker/generics.rs`). Pattern binding
reads those declared types directly and binds the ident to them:

    // checker/scope.rs::bind_pattern_ty
    let Some(def) = adt.variants.iter().find(|v| v.name == name.name) else { … };
    // ...
    let field_tys = def.fields.clone();
    for (field_ty, pat) in field_tys.iter().zip(fields) {
        self.bind_simple_pattern(pat, field_ty)?;
    }

No substitution of `T` for the concrete type argument in `scr_ty`
happens. For a *monomorphic* ADT (`type Color = | RGB(Int) | HSL(Int)`)
this is fine — the declared fields already carry the concrete types.
For a generic ADT (`type Box[T] = | Box(T)`) the field type stays `T`
in the binding, so any function that expects the payload to be `Int`
(when the caller passed a `Box[Int]`) fails with `expected Int, got T`.

## Reproduction

    type Box[T] =
      | Box(T)

    fn unbox(b: Box[Int]) -> Int {
      match b {
        Box(x) => x
      }
    }

`flux-types::type_check` fails with `expected `Int`, got `T``.

The same function against a monomorphic ADT works:

    type BoxI = | BoxI(Int)
    fn unbox(b: BoxI) -> Int { match b { BoxI(x) => x } }

## Impact

Any generic ADT (the standard `Result[T, E]`, `Option`, user-defined
`Box[T]`, `Tree[T]`, …) whose value flows into a `match` arm and whose
payload is used at the concrete instantiation site fails to
type-check. The construction side already works (`Box(1)` compiles),
so users hit the failure only when they try to *use* the value.

## What a fix needs

`bind_pattern_ty` must, for a `TcType::Variant(adt_name, type_args)` or
`TcType::Named(adt_name, type_args)` scrutinee type, substitute the
ADT's declared generic parameter names with the concrete `type_args`
before binding the field patterns.

Concretely:

1. `AdtDef` (in `env.rs`) already carries `params: Vec<String>`. Build
   a `HashMap<String, TcType>` mapping each `params[i]` to
   `type_args[i]` (or to a fresh variable when the scrutinee's type
   is a generic variable).
2. Before `bind_simple_pattern`, substitute the field type through
   that map. Reuse the recursion shape of `rewrite_generics` (which
   already walks every composite `TcType`) — pass a
   `&HashMap<String, TcType>` instead of the current
   `HashMap<String, usize>`.

The substitution helper is the same shape as the existing
`rewrite_named_to_var`; the fix is a two-line call site change plus a
small helper.

## Regression test

`crates/flux-parity/tests/parity.rs::generic_adt_payload_binding_is_a_known_gap`
— added in this commit. Asserts the current failure
(`expected Int, got T`). When the substitution lands, un-ignore and
assert successful compile.

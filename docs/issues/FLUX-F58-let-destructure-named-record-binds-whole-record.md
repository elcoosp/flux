# FLUX-F58 — `let { x, y } = p` binds every field to the whole record for Named types

**Status:** OPEN (documented; not fixed — nontrivial, needs the nominal
record's field types from the env)
**Found:** round-103 bug hunt (`flux-types/checker/scope.rs`)
**Severity:** medium (a user who destructures a nominal record —
`record R { x: Int, y: Int }` + `let { x, y } = make()` — gets a `Named`
binding for both `x` and `y` that carries the whole record type `R`,
not the field types `Int`. Any arithmetic or string operation on the
bound names fails with a misleading "operator Add requires a Numeric
type, got `R`".)

## Summary

`crates/flux-types/src/checker/scope.rs::bind_record_let`:

    let value_ty = self.resolve(value_ty);
    for field in fields {
        let found = match &value_ty {
            TcType::Record(fs) => fs.iter().find(|(n, _)| n == &field.name).map(|(_, t)| &**t),
            TcType::Named(_, _) => Some(&value_ty),        // <-- HERE
            _ => None,
        };
        // ...
        let scheme = generalise(ty, &env_free);
        self.env.insert(field.name.clone(), Binding::Poly(scheme));
    }

For a *structural* `TcType::Record`, the match correctly finds the
field's type. For a *nominal* `TcType::Named("R", _)` (a `record R { … }`
declaration), the match returns the whole record type `R` as the type
of **every** field. So `let { x, y } = p` binds both `x` and `y` to
`R`, not to `Int`.

## Reproduction

    record R { x: Int, y: Int }

    fn make() -> R { R { x: 1, y: 2 } }

    compo C
      let { x, y } = make()
      state total: Int = x + y      // <- fails: `Add` requires Numeric, got `R`
      Text("x")

`flux_types::type_check` fails with:

    operator `Add` requires a Numeric type, got `R`

A structural record path works:

    let p = { x: 1, y: 2 }           // structural
    let { x, y } = p
    x + y                             // OK

## Impact

Any user who declares a nominal record (`record R { … }`) and
destructures it in a `let` gets bindings typed as the record. Field
access on the destructured name is `R.x` (the record's own field), not
the field's value. This makes record destructuring effectively broken
for nominal types.

`record R { x: Int, y: Int }` is a first-class surface form (see
`parser::record_decl`), so a user following the spec has no way to
destructure a record value.

## What a fix needs

The binder needs to look up the nominal record's *registered fields*
from the environment and match each pattern name against them — the
same information the `TcType::Record` arm already provides for
structural records.

`Env` already stores this under
`Binding::Ctor(CtorKind::Record { fields })` (see `checker/generics.rs`
`collect_adts`). So the fix is:

    TcType::Named(name, _) => {
        match self.env.lookup(name) {
            Some(Binding::Ctor(CtorKind::Record { fields })) => {
                fields.iter().find(|(n, _)| n == &field.name).map(|(_, t)| t)
            }
            _ => Some(&value_ty),   // opaque fallback
        }
    }

The `Named` arm also currently handles ADT variants
(`TcType::Named(adt_name, _)` where `adt_name` is a sum type), which
have no `Record` binding; those fall through to `_ => None`, which the
current arm swallows by returning `Some(&value_ty)` — hence the
fallback above preserves current behavior for non-record Named types.

## Regression test

`crates/flux-parity/tests/parity.rs::named_record_destructure_is_a_known_gap`
— added in this commit. Asserts the current failure
(`operator Add requires a Numeric type, got R`). When the fix lands,
un-ignore and assert the program compiles.

## Related

* **F52** — a *similar* type-substitution gap in generic-ADT pattern
  bindings. Both are in the field-type resolution path and should be
  addressed together.

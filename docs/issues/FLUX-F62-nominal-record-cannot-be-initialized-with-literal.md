# FLUX-F62 — A nominal `record` type cannot be initialized from a `{ … }` literal

**Status:** OPEN (documented; not fixed — the fix is a design call)
**Found:** round-109 bug hunt (type-checker probes)
**Severity:** medium (a user declaring `record P { x: Int, y: Int }` cannot
construct a `P` value via `P(x: 1, y: 2)` (the state/let annotation
rejects the result) **nor** via `{ x: 1, y: 2 }` (the structural record
literal has a different type). The declaration syntax is accepted; every
way of building a value of that type is rejected.)

## Summary

`crates/flux-parser` accepts `record P { x: Int, y: Int }` (see
`record_decl`), and the type checker registers it as a nominal
constructor:

    // checker/generics.rs::collect_adts
    Decl::Record(rec) => {
        env.insert(
            rec.name.name.clone(),
            Binding::Ctor(CtorKind::Record { fields }),
        );
    }

That constructor is callable (`apply_callee` handles
`CalleeShape::Record`), but its *return type* is `Named(name, [])`:

    // checker/apply_callee.rs
    Ok(TcType::Named(name.clone(), Vec::new()))

By contrast, an anonymous record literal `{ x: 1, y: 2 }` has a
structural type:

    // checker/infer.rs
    ExprKind::Record { … } => Ok(TcType::Record(field_tys))

`TcType::Named("P", [])` and `TcType::Record([(x, Int), (y, Int)])`
never unify, so every plausible way of initialising a nominal record
value fails.

## Reproduction

Case 1 — initialisation with a literal:

    record P { x: Int, y: Int }

    compo C
      state p: P = { x: 0, y: 0 }         // <- fails
      Text("x")

    error: expected `P`, got `{ x: Int, y: Int }`

Case 2 — construction via the nominal call:

    record P { x: Int, y: Int }

    compo C
      state p: P = P(x: 0, y: 0)          // <- this is OK
      Text("x")

The call produces `Named("P", [])` — matches — but the value is a
`Value::Record` at runtime, and a consumer that reads `p.x` would
succeed (field lookup on a Named record type resolves through
`CtorKind::Record` — see `field.rs:77`).

Case 3 — anonymous record works but is structural:

    compo C
      state p: { x: Int, y: Int } = { x: 0, y: 0 }  // <- works
      Text("x")

The structural form has no nominal name, so no mismatch.

## Impact

A user who follows the surface grammar (`record Name { … }`) cannot
write a value of that type without an explicit constructor call, and
the type error names the two types as if they were unrelated
(`expected P, got { x: Int, y: Int }`). The distinction between
nominal and structural records is invisible in the docs; the fix
should either accept the literal as a `P` (structural → nominal
coercion at the annotation site) or explain the difference with a
hint.

## What a fix needs

Three coherent options:

1. **Structural-to-nominal coercion.** When `expect(&Named("P", []), &Record(fields))`
   runs, look up `P`'s registered fields in the environment and
   unify them structurally. This makes case 1 work and matches how
   the parity reducer already treats the two forms identically (see
   `flux-parity::reduce::node_from_ast`).

2. **Nominal-to-structural unification.** Treat `Named("P", [])` and
   `Record(fields)` as interchangeable wherever the field lists match.
   More invasive but consistent.

3. **Reject the `record` keyword and require `type P = { … }` (structural).**
   Remove the nominal declaration syntax if it offers nothing over the
   structural form. This is a design decision about whether Flux needs
   nominal records at all.

Option 1 is the smallest, most targeted fix; the coercion lives inside
`expect`/`unify_into`, where the nominal name is available.

## Regression tests

`crates/flux-parity/tests/parity.rs::nominal_record_literal_init_is_a_known_gap`
— added in this commit. Asserts the current failure
(`expected P, got { x: Int, y: Int }`). When the fix lands, un-ignore
and assert successful compilation.

## Related

* **F58** — `let { x, y } = p` binds the whole record for named
  records (a different nominal-vs-structural gap on the destruct
  side). Both should be addressed by the same design decision.

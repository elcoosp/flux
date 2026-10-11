# FLUX-F22 — `label`/`color`/`alignment` props are not emitted by any codegen backend

**Status:** OPEN
**Found:** round-86 bug hunt
**Severity:** medium (silent fidelity loss: a Flux program using these props
compiles and runs, but the generated native UI ignores the prop entirely)

## Summary

The dev-path reducer (`crates/flux-parity/src/reduce.rs::props_from_args`)
tracked three named props — `label`, `color`, `alignment` — as part of the
reduced `ViewNode::Primitive` structure. Its doc comment asserted these
"survive as a named argument in both backends' generated source".

They do not. The codegen emitter's container branch
(`crates/flux-codegen-core/src/emitter.rs`, `PrimitiveKind::Container`) reads
only the `gap` prop. `alignment`, `color` and `label` are collected into the
props map by `collect_props`, then never referenced — silently dropped from
the emitted Swift/Kotlin source.

## Reproduction

    compo C
      Column(alignment: "a") { Text("x") }

Reduced dev tree:   `Column { props: [("alignment", "\"a\"")], children: [Text] }`
Reduced Swift tree: `Column { props: [], children: [Text] }`
Reduced Kotlin tree: `Column { props: [], children: [Text] }`

`check_parity` reports `is_equivalent() == false`.

## Impact

A user writing `Column(alignment: "center")` gets a native `VStack`/`Column`
with default alignment — the prop is invisible. Same for `color:` and
`label:` on any primitive.

## What a fix needs

* Add native-emission support in each backend for these props where the host
  surface accepts them (`VStack(alignment: .leading, …)` for Swift,
  `Column(horizontalAlignment = Alignment.CenterHorizontally, …)` for Kotlin,
  plus color maps).
* Update the recognizer (`recognize_swift::swift_views::extract_swift_props`)
  to read the emitted spellings and normalize them to the dev-side canonical
  form (`"center"`, `"leading"`, …).
* Restore `["label", "color", "alignment"]` to the dev reducer's `RECOGNISED`
  list and drop the "not currently emitted" workaround below.

## Interim mitigation (round 86)

The dev reducer's `RECOGNISED` list is emptied so the dev and release reduced
trees agree on "no props" today. This restores parity green without losing the
fidelity bug from view — the fix is the above.

## Related

* `recognize_swift::swift_views::extract_swift_props` carries the same
  `RECOGNISED` list; it is dead code today because no backend emits these props.

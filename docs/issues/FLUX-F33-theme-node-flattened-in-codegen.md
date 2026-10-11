# FLUX-F33 — `Theme { … }` is flattened by both codegen backends

**Status:** OPEN (documented; not fixed this round)
**Found:** round-86 bug hunt (primitive probes)
**Severity:** medium (parity-only divergence: a Flux program using the
documented `Theme { … }` wrapper — the FLUX-043 design-token theming
primitive — fails `check_parity` because both codegen backends elide
the wrapper entirely while the dev reducer keeps the `Theme` node)

## Summary

The Flux `Theme` primitive is intended to open a design-token theming
scope whose children render with the theme's tokens. The dev-path
reducer records it as `Primitive { name: "Theme", children: [ … ] }`.

The release-side emitters (`flux-codegen-swift::backend_impl::emit_…`
and `flux-codegen-kotlin::backend_impl::emit_…`) fold the wrapper away:
the children are emitted directly at the enclosing level and the
generated code refers to `FluxTheme.colorPrimary` statically instead.
The recognizers therefore see `Text` as a sibling of the wrapping
component, with no `Theme` node anywhere — parity diverges.

The `normalize_view_name` table already folds `MaterialTheme` /
`FluxTheme` → `Theme`, but the node is never emitted, so the fold is
dead for this case.

## Reproduction

    compo C
      Theme {
        Text("x")
      }

Reduced trees:

* dev: `Primitive { name: "Theme", children: [Text] }`
* swift: `Primitive { name: "Text" }` (Theme gone)
* kotlin: `Primitive { name: "Text" }` (Theme gone)

`check_parity` reports `is_equivalent() == false`.

## What a fix needs

Two options:

1. **Make the emitters keep the wrapper.** Emit a native theming scope
   (`FluxTheme { … }` on Swift, `CompositionLocalProvider(…){ … }` on
   Kotlin) so the recognizer can recover a `Theme` node. This is the
   faithful translation of "children render inside the theme scope".

2. **Make the dev reducer elide the wrapper**, matching the current
   lossy codegen. Add `Theme` to the same wrapper-elision set the
   equivalence relation already uses for single-child `Column`/`Row`
   wrappers (see `equivalence::elide_wrappers`). The information loss is
   then visible on all three sides and parity holds.

Option 2 is cheap and matches the existing `Switch`/`Checkbox` collapse
precedent (F32). Option 1 is more faithful but is a cross-backend
feature. Filed rather than fixed because the choice is a design call.

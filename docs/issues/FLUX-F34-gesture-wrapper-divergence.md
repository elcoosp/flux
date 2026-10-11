# FLUX-F34 — `Gesture { … }` produces three different shapes across the paths

**Status:** OPEN (documented; not fixed this round)
**Found:** round-86 bug hunt (primitive probes)
**Severity:** medium (parity-only divergence: the same Flux program
yields `Gesture` on dev, a `Column` on Swift, and a `Stack` on Kotlin)

## Summary

The Flux `Gesture(onTap: …) { children }` primitive is a wrapper that
attaches a gesture recognizer to its children. The dev reducer records
it as `Primitive { name: "Gesture", children: [ … ] }`.

The release-side emitters disagree on the wrapper's native name and
kind:

* Swift emits `Column { children }` (a vertical layout that does not
  carry the gesture at all).
* Kotlin emits `Stack { children }` (a layout container).

The recognizers read the emitted surface — `Column` on Swift,
`Stack` on Kotlin — and `normalize_view_name` does not fold either back
to `Gesture`. Parity diverges.

## Reproduction

    compo C
      Gesture(onTap: { }) {
        Text("x")
      }

Reduced trees:

* dev: `Primitive { name: "Gesture", children: [Text] }`
* swift: `Primitive { name: "Column", children: [Text] }`
* kotlin: `Primitive { name: "Stack", children: [Text] }`

`check_parity` reports `is_equivalent() == false`.

## What a fix needs

Decide the intended native shape:

1. **A container wrapper.** Both backends emit the same native layout
   wrapper (`Column` on both) and the recognizers fold it back to
   `Gesture` via `normalize_view_name`. The gesture handler itself
   stays attached in emitted code (Swift `.gesture(...)`, Kotlin
   `.pointerInput { … }`) — that part is separate from the structural
   parity question, which is only about the view name.

2. **An adapter.** A real `Gesture` adapter is registered on both
   platforms (the codegen registry at
   `flux-codegen-core/src/primitives.rs:826` declares
   `kotlin_adapter: Some("GestureAdapter")`). Then the recognizer sees
   `GestureAdapter`/`Gesture` on both sides and the normalizer folds
   them to `Gesture`.

Whichever is chosen, the two backends must emit the *same* wrapper
name so the recognizers agree.

## Regression test

Filed at `crates/flux-parity/tests/parity.rs`:
`gesture_wrapper_is_a_known_gap` — `#[ignore]`d, asserts the current
divergence (dev has `Gesture`, Swift has `Column`, Kotlin has `Stack`).

# FLUX-F35 — `Picker` and `DatePicker` release names do not normalize

**Status:** OPEN (fix landed in same commit)
**Found:** round-86 bug hunt (full-primitive parity probe)
**Severity:** low (parity-only divergence: the primitives type-check
and run, but `check_parity` fails because the Kotlin backend emits
different native names for them)

## Summary

The primitive registry (`crates/flux-codegen-core/src/primitives.rs`)
declares `Picker` and `DatePicker` as leaves with a `value` primary prop
and an `onChange` handler prop. Both codegen backends emit them via the
`form_control` fallback (no dedicated `form_control` arm — the
comment says "Picker / DatePicker need option lists and date state we do
not reconstruct here; fall back to the legacy call").

The two backends then disagree on the emitted native name:

* `Picker`: Swift emits `Picker`, Kotlin emits `DropdownMenu`. The
  recognizers read them back verbatim; `normalize_view_name` did not
  fold `DropdownMenu` → `Picker`, so a Flux `Picker(...)` diverges.
* `DatePicker`: Swift emits `DatePicker`, Kotlin emits
  `DatePickerDialog`. Same story.

## Reproduction

    compo C
      state v: Int = 0
      Picker(value: v, onChange: { v = 1 })

Reduced trees: dev `Picker`, Swift `Picker`, Kotlin `DropdownMenu`.

    compo C
      state v: Int = 0
      DatePicker(value: v, onChange: { v = 1 })

Reduced trees: dev `DatePicker`, Swift `DatePicker`, Kotlin
`DatePickerDialog`.

Both `check_parity` calls report `is_equivalent() == false`.

## Fix

Add `DropdownMenu` → `Picker` and `DatePickerDialog` → `DatePicker` to
`flux-codegen-core::view_tree::normalize_view_name`, matching the F32
precedent (`Toggle`, `TextEditor`/`OutlinedTextField`). The two backends
remain free to emit their native shape; the reduced trees compare equal.

## Related

* F32 — same class for `Checkbox`/`Toggle` and `TextArea`/`TextEditor`/
  `OutlinedTextField`.
* F33/F34 — same class for `Theme`/`Gesture`, filed because the fix
  requires a design decision rather than just a normalizer entry.

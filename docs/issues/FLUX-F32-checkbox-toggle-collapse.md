# FLUX-F32 — Swift collapses `Checkbox` to `Toggle`; parity normalizer must match

**Status:** OPEN (fix landed in same commit)
**Found:** round-86 bug hunt (primitive probes)
**Severity:** low (parity-only divergence: a Flux program using
`Checkbox(...)` type-checks and runs, but `check_parity` fails because
the Swift backend emits a SwiftUI `Toggle` for it while the dev side
still says `Checkbox`)

## Summary

The Swift emitter (`flux-codegen-swift::backend_impl::form_control`)
maps `Switch | Checkbox | Toggle` all onto SwiftUI's `Toggle(isOn:)` —
SwiftUI has no distinct checkbox control, and the emitter chose the
lossy translation rather than a `.toggleStyle(.checkbox)` modifier. The
Kotlin emitter keeps `Checkbox` distinct (`Checkbox(checked = …)`).

The parity normalizer (`flux-codegen-core::view_tree::normalize_view_name`)
already folds `Switch` → `Toggle` so both backends' `Switch`-from-Flux
output compares equal. It did **not** fold `Checkbox` → `Toggle`, so a
Flux `Checkbox(value:on, onChange:…)` produced:
* dev: `Checkbox`
* swift: `Toggle`
* kotlin: `Checkbox`

`check_parity` reported a divergence.

## Fix

Add `Checkbox` → `Toggle` to `normalize_view_name`, matching the
existing `Switch` → `Toggle` entry and the Swift emitter's existing
lossy translation. The reduced tree then reads `Toggle` on all three
sides.

The cost is the same as the existing `Switch` collapse: a Flux program
using `Checkbox` and a Flux program using `Toggle` produce identical
reduced trees, so structural parity can no longer distinguish them. The
information loss is already present in the emitted Swift; the normalizer
only makes it visible on both sides.

A future emitter fix (emit `Toggle(...).toggleStyle(.checkbox)` on
platforms that support it, or a Flux-specific checkbox wrapper) would
let this normalizer entry be removed and the distinction restored.

## Related normalizer entry

`TextEditor` → `TextArea` and `OutlinedTextField` → `TextArea` were added
in the same commit — both are emitted uniquely for the Flux `TextArea`
primitive (Swift `TextEditor(text:)`, Kotlin
`OutlinedTextField(value=…, singleLine = false)`) and neither previously
normalized back, so any Flux program using `TextArea` failed parity.

## Regression test

`crates/flux-parity/tests/parity.rs::form_control_primitive_names_round_trip`
exercises `Checkbox`, `TextArea`, `Switch`, `Slider`, `Toggle` and
asserts parity across all three.

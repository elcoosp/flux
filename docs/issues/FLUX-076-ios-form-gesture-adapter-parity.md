---
id: FLUX-076
status: done
lane: LANE-N
phase: "Phase 2"
blocked_by: [FLUX-040, FLUX-041]
labels:
  - stdlib
  - primitive
  - ios
  - parity
source: FLUX-040 / FLUX-041 parity gate (AGENTS.md — a primitive needs BOTH adapter kits before advertising)
related_adrs:
  - ADR-0047
---

# FLUX-076: iOS adapter parity for FLUX-040 form + FLUX-041 gesture primitives

## Problem Statement

FLUX-040 (form primitives) and FLUX-041 (gestures) have landed on **Android +
the stdlib** but are **not yet advertised** to authors. AGENTS.md is explicit:
*a primitive needs BOTH adapter kits before advertising.* iOS
(`adapters/ui-swift`) still has no `Switch` / `Checkbox` / `Slider` / `Picker` /
`DatePicker` / `TextArea` / `Gesture` adapters, so the seven primitives cannot be
seeded into the Swift prelude / `prelude.flux` public surface or documented as
generally available.

The Android side that already landed (the reference contract to mirror):

- `adapters/ui-kotlin`: `SwitchAdapter`, `CheckboxAdapter`, `SliderAdapter`,
  `PickerAdapter`, `DatePickerAdapter`, `TextAreaAdapter` (FLUX-040) and
  `GestureAdapter` (FLUX-041, a container that reconciles children by stable
  node id and declares its `kind` + `onGesture` handler as view properties).
- `PropsIndex.kt`: each prop index is derived from the **FNV-1a-32 name digest**
  (`propIndexForName`), never a hardcoded position (AGENTS.md §3.2). The Swift
  kit must derive the same indices identically.
- `stdlib/`: `switch.flux`, `checkbox.flux`, `slider.flux`, `picker.flux`,
  `date_picker.flux`, `text_area.flux`, `gesture.flux` — `compo` declarations
  with the `value`/`onChange` (+ `kind`/`onGesture`) signal contract.
- Per-node adapter factories (no shared singletons — FLUX-007), weakly-held
  executor, handler dispatch through `FluxExecutor.dispatch`.

## Solution

Port the seven adapters to `adapters/ui-swift` following the existing Swift dev
adapter contract (the `FluxUIKit`/`FluxAdapter` equivalents). Map each to its
native control:

- `Switch` → `Toggle`, `Checkbox` → `Toggle`, `Slider` → `Slider`,
  `Picker` → `Picker`, `DatePicker` → `DatePicker`, `TextArea` → `TextEditor`
  (the `kotlin_view`/`swift_view` spellings already recorded in
  `flux-codegen-core/src/primitives.rs`).
- `Gesture` → a `VStack`/`Container` carrying the matching
  `UIGestureRecognizer` (longPress / swipe / drag / pinch), reconciling children
  by stable node id.

Resolve prop indices through the same FNV-1a name digest so the iOS host stays
in lockstep with the dev server. Add the Swift-side JVM/XCTest equivalents of
`FormGestureAdapterTest.kt` (prop mapping, handler binding, executor disposal
no-op, `Gesture` keyed reconciliation).

## Implementation Decisions

- Mirror the Android prop-index set exactly (same names → same derived indices).
- Do **not** advertise the primitives (seed `prelude.flux` / public surface)
  until this issue is `done`; the FLUX-040/041 docs carry the parity gate.

## Testing Decisions

- XCTest (or the Swift kit's equivalent) asserting each adapter's dev/release
  mapping and that a `Switch` toggle / `Gesture` fire dispatches the bound
  handler — parity with the Android `FormGestureAdapterTest`.

## Out of Scope

- The Android side (already landed), the stdlib `.flux` sources (already
  landed), and the signal-graph animation primitive (FLUX-042).

## Implementation Note (2026-08-30)

**Landed — both kits now exist, the FLUX-040/041 parity gate is cleared.**

- `adapters/ui-swift`: seven declarative adapters mirroring the Android shapes
  exactly, all resolving prop indices through the shared FNV-1a `Props.propIndex`
  digest (no hardcoded positions, AGENTS.md §3.2): `SwitchAdapter` (`UISwitch`),
  `CheckboxAdapter` (`UIButton` checkmark — UIKit has no native checkbox),
  `SliderAdapter` (`UISlider`), `PickerAdapter` (`UIPickerView` + retained
  delegate), `DatePickerAdapter` (`UIDatePicker` .date), `TextAreaAdapter`
  (`UITextView` + retained delegate), and `GestureAdapter` (`UIView` container
  that attaches the matching `UIGestureRecognizer` by `kind` and reconciles
  children by stable view identity).
- `Props.swift`: added `getList(named:)` so `Picker` reads its `items` the same
  way the Android `Props.getList` does.
- `runtimes/ios/FluxHost/Sources/FluxHost/AdapterKit.swift`: registered all
  seven in the `AdapterRegistry` factory table (keyed by node name), matching
  the existing primitive registrations.
- XCTest: `FormGestureAdapterTests.swift` pins update/handler/executor-disposal/
  keyed-reconciliation — parity with the Android `FormGestureAdapterTest`. The
  `:adapters:ui-swift` test target builds + runs on the iOS Simulator (60
  tests green), and the `FluxApp` host compiles with the new registrations.

Both adapter kits now implement FLUX-040/041, so the primitives satisfy
AGENTS.md's "both kits before advertising" rule — they can be seeded into the
public surface / `prelude.flux` now.

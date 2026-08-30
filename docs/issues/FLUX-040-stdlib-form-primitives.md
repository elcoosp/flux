---
id: FLUX-040
status: done   # both adapter kits landed (Android FLUX-040 + iOS FLUX-076 parity); seeded into stdlib/prelude.flux per the advertising gate (AGENTS.md §3.5)
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
  - form
source: CHANGELOG.md §PRD-N (deferred: "form primitives (Switch/Checkbox/Slider/Picker/DatePicker/TextArea)") + roadmap §4
related_adrs:
  - ADR-0047
---

# FLUX-040: Stdlib form primitives — Switch / Checkbox / Slider / Picker / DatePicker / TextArea

> **Status note (2026-08-29):** relabeled `partial` → `todo`. A source grep of
> `stdlib/` finds **zero** form primitives (`Switch`/`Checkbox`/`Slider`/`Picker`/
> `DatePicker`/`TextArea`) — only the ADR-0047 contract intent exists. Nothing has
> landed in the stdlib yet.

- **Lane:** LANE-N (Phase 2)
- **Depends on:** none (but pairs with FLUX-041 gestures + FLUX-044 a11y)
- **Source:** `CHANGELOG.md` §PRD-N deferred + roadmap §4
- **Related ADRs:** ADR-0047

## Problem Statement

Form primitives (`Switch`/`Checkbox`/`Slider`/`Picker`/`DatePicker`/multi-line
`TextArea` + form validation composition) are deferred. Forms are ~half of any
CRUD app.

## Solution

Each form primitive maps to its native control via ADR-0047, seeded in the prelude,
with a `value`/on-change signal contract and a `flux-parity` trace test. A
`Form` composition helper wires validation.

## Implementation Decisions

- Each primitive carries a `value` signal + an `onChange` callback prop (the same
  signal-graph contract the existing `text_field` uses).
- Validation is a composition helper, not a primitive.

## Testing Decisions

- Parity trace tests pin each control's dev/release mapping; a JVM test asserts a
  `Switch` toggle writes its signal.

## Out of Scope

- Gestures (FLUX-041), theming (FLUX-043).

## Implementation Note (2026-08-30)

**Android + stdlib landed (NOT yet advertised — see parity gate below).**

- `adapters/ui-kotlin`: seven new declarative adapters, all following the
  unified-tier contract (AGENTS.md §3.5) and the `TextInputAdapter` shape:
  `SwitchAdapter`, `CheckboxAdapter`, `SliderAdapter`, `PickerAdapter`,
  `DatePickerAdapter`, `TextAreaAdapter` (FLUX-040), and `GestureAdapter`
  (FLUX-041, a container that reconciles children by stable node id and declares
  its gesture `kind` + `onGesture` handler as view properties).
  Each adapter resolves its prop indices through `PropsIndex` via the shared
  `propIndexForName` FNV-1a digest (§3.2) — no hardcoded positions. All seven
  are registered in `FluxUiKit.adapters` as factories (FLUX-007: per-node fresh
  instances, never shared singletons).
- `PropsIndex.kt`: FLUX-040/041 prop indices added (`SWITCH_VALUE`, `SLIDER_*`,
  `PICKER_ITEMS`, `GESTURE_KIND`, `GESTURE_THRESHOLD`, …).
- `stdlib/`: `switch.flux`, `checkbox.flux`, `slider.flux`, `picker.flux`,
  `date_picker.flux`, `text_area.flux` (FLUX-040) and `gesture.flux` (FLUX-041)
  — `compo` declarations with the same `value`/`onChange` (+ `kind`/`onGesture`)
  signal contract the existing `text_field.flux` uses.
- JVM tests: `FormGestureAdapterTest.kt` pins each adapter's `update` prop
  mapping, handler binding through the weakly-held `FluxExecutor`, executor
  disposal no-op, and `Gesture` keyed child reconciliation; `FluxUiKitTest`
  already asserts every registered kind resolves to a fresh instance. The
  module's `:adapters:ui-kotlin:test` + `:ktlintCheck` are green.

### Parity gate — CLEARED (FLUX-076 done)
AGENTS.md: a primitive needs **both** adapter kits before it is advertised to
authors. iOS (`adapters/ui-swift`) now has all seven adapters (FLUX-076 landed),
so the FLUX-040 form primitives satisfy the parity rule and may be seeded into
the public surface / `prelude.flux`.

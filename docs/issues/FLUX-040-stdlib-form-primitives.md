---
id: FLUX-040
status: partial
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

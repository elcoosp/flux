---
id: FLUX-037
status: partial
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
  - layout
source: CHANGELOG.md §PRD-N (deferred: "Stack/Grid/Spacer/SafeArea")
related_adrs:
  - ADR-0047
---

# FLUX-037: Stdlib layout primitives — Stack / Grid / Spacer / SafeArea

> **Status note (2026-08-29):** relabeled `done` → `partial`. A source grep of
> `stdlib/` finds **zero** of these primitives, and neither host has an adapter
> branch (`adapters/ui-swift` has only `ColumnAdapter`/`RowAdapter`/`Alignment`;
> Android `LinearAdapters` covers Column/Row only). Only `flux-codegen-core`
> template scaffolding references them. A Flux app cannot express or render
> Stack/Grid/Spacer/SafeArea yet — stdlib declaration + host adapter + test missing.

- **Lane:** LANE-N (Phase 2)
- **Depends on:** none (PRD-N `ScrollView` slice is the template)
- **Source:** `CHANGELOG.md` §PRD-N deferred (remaining PRD-N families)
- **Related ADRs:** ADR-0047 (single-source primitive registry)

## Problem Statement

PRD-N shipped only `ScrollView`. The roadmap (§4) lists `Stack` (z-order overlay),
`Grid`, `Spacer`, `SafeArea` as the next most-common layout primitives — all
deferred. A CRUD/social app cannot be built without them.

## Solution

Each primitive follows the PRD-N `ScrollView` template: register in the ADR-0047
codegen primitive registry as a `Container`/`Leaf` mapping to the right Kotlin +
Swift backend, seed in the `flux-types` prelude, declare in stdlib, and add a
`flux-parity` dev/release trace test. Because iOS is still imperative (ADR-0048,
Axis 2), build against **one** rendering model per the convergence decision — do
not hand-duplicate UIKit vs SwiftUI mapping beyond what the kit provides.

## Implementation Decisions

- One primitive per PRD-N slice issue? No — group the four layout primitives here
  (same registry family) but keep each with its own parity test.
- a11y props threaded from day one (roadmap §4 "Theming & accessibility").

## Testing Decisions

- Parity guard: `registry_covers_every_prelude_primitive` + `registry_has_no_unknown_entries`
  extended for each; a `flux-parity` trace test pins dev/release mapping.

## Out of Scope

- `Modal`/`Sheet`/`Dialog` (FLUX-038), `Image` (FLUX-039), form primitives
  (FLUX-040), gestures (FLUX-041), animation (FLUX-042), theming (FLUX-043),
  a11y (FLUX-044).

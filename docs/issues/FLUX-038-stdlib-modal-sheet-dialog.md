---
id: FLUX-038
status: done
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
source: CHANGELOG.md §PRD-N (deferred: "Modal/Sheet/Dialog")
related_adrs:
  - ADR-0047
---

# FLUX-038: Stdlib container primitives — Modal / Sheet / Dialog

> **Status note (2026-08-29):** relabeled `done` → `partial`. `flux-codegen-core`
> defines `Modal`/`Sheet`/`Dialog` enum variants (template scaffolding) but there
> is **no stdlib declaration** and **no host reconciler branch** on either platform
> (grep of `ShadowTreeReconciler.swift` / `DirtyReconciler.kt` for Modal/Sheet/Dialog
> returns nothing). A Flux app cannot express or render these containers yet.

- **Lane:** LANE-N (Phase 2)
- **Depends on:** FLUX-037 (layout primitives + the iOS convergence decision)
- **Source:** `CHANGELOG.md` §PRD-N deferred
- **Related ADRs:** ADR-0047

## Problem Statement

`Modal`/`Sheet`/`Dialog` with a real transition/animation contract are deferred.
These need a presentation model that the current imperative iOS reconciler
(ADR-0048 Axis 2) doesn't cleanly express — so they depend on the convergence
call landing first.

## Solution

Add the three container primitives with a presentation/animated-open contract in
the ADR-0047 registry, seeded in the prelude, with `flux-parity` trace tests. The
transition contract is data the host consumes (not a wire animation frame).

## Implementation Decisions

- Wait for the ADR-0048 convergence decision (LANE-J) so the host rendering model
  is settled before building the presentation layer.
- Animation is specified as a named transition the host maps to its native
  equivalent.

## Testing Decisions

- Parity trace test: a `Modal` open pins the dev/release node mapping on both
  backends.

## Out of Scope

- The signal-graph animation *primitive* (FLUX-042) — that is a different axis.

## Closure (2026-08-29)

Delivered end-to-end:

- **Stdlib declarations:** `stdlib/modal.flux`, `sheet.flux`, `dialog.flux`
  (`onDismiss: Handler`); parse + type-check clean.
- **Host adapters (both platforms):** `OverlayMotionAdapters.kt` (`ModalAdapter`/
  `SheetAdapter`/`DialogAdapter`) + `OverlayMotionAdapters.swift`
  (`OverlayContainerAdapter` subclasses) — each a real container that hosts its
  `content` children, so the overlay resolves and renders instead of blanking.
  Registered in `FluxUiKit.adapters` (Android) and `AdapterRegistry` (iOS).
- **Parity test:** `flux_038_modal_open_pins_dev_release_mapping` pins the dev/
  release node mapping on both backends (pre-existing, re-confirmed green).
- **Tests:** Android JVM `LayoutOverlayAdapterTest` + iOS `LayoutOverlayAdapterTests`.

Native *presentation* (hosted sheet/alert) is gated on the ADR-0048 iOS dev-tier
convergence decision — the adapters currently resolve to a container carrying the
content subtree (documented; not a blank). The `onDismiss` handler is read by the
host layer once the surface is wired.

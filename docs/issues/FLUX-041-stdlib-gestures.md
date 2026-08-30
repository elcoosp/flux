---
id: FLUX-041
status: partial
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
  - gesture
source: CHANGELOG.md §PRD-N (deferred: "gestures") + roadmap §4
related_adrs:
  - ADR-0047
---

# FLUX-041: Stdlib gesture primitives — long-press / swipe / drag / pinch

> **Status note (2026-08-29):** relabeled `partial` → `todo`. A source grep of
> `stdlib/` finds **zero** gesture primitives (`onLongPress`/`onSwipe`/`onDrag`/
> `onPinch`) — only the ADR-0047 contract intent exists. Nothing has landed in the
> stdlib yet.

- **Lane:** LANE-N (Phase 2)
- **Depends on:** PRD-N `ScrollView` (scroll already uses pan)
- **Source:** `CHANGELOG.md` §PRD-N deferred + roadmap §4
- **Related ADRs:** ADR-0047

## Problem Statement

`Gesture` primitives (tap already exists via `onClick`; add long-press, swipe,
drag, pinch) are deferred. These are core to "feels native."

## Solution

A `Gesture` wrapper carrying a gesture kind + callback prop, mapping to
`UIGestureRecognizer` (iOS) / `Modifier.pointerInput` (Android) via ADR-0047, with
`flux-parity` trace tests for the attach/detach lifecycle.

## Implementation Decisions

- Reuses the existing `onClick` callback-prop contract for consistency.
- Drag/pinch surface continuous deltas as a signal stream where useful.

## Testing Decisions

- Parity trace test: a `Gesture(kind: longPress)` attaches the expected recognizer
  on both backends.

## Out of Scope

- The signal-graph animation primitive (FLUX-042).

## Implementation Note (2026-08-30)

**Android + stdlib landed (NOT yet advertised — see parity gate below).**

- `adapters/ui-kotlin`: `GestureAdapter` (a container) maps a `Gesture` node to a
  native gesture surface. It reconciles its child subtree by stable node id
  (keyed reconciliation, FLUX-007) and declares the gesture `kind`
  (longPress/swipe/drag/pinch) + `onGesture` handler as view properties; drag/
  pinch surface a continuous `threshold` as a host-render-only property. The
  native recognizer attach/detach is host-side. It follows the unified-tier
  contract (AGENTS.md §3.5) and reuses the `onClick` handler-prop convention.
  Registered in `FluxUiKit.adapters` as a factory.
- `PropsIndex.kt`: `GESTURE_KIND`, `GESTURE_ON_GESTURE`, `GESTURE_THRESHOLD`
  added via the shared `propIndexForName` FNV-1a digest (§3.2).
- `stdlib/gesture.flux`: a `compo` wrapper carrying `kind`/`onGesture`/`threshold`
  — the gesture surface over a caller-supplied child subtree.
- JVM tests: `FormGestureAdapterTest.kt` pins `Gesture` kind/threshold mapping,
  handler binding through the weakly-held `FluxExecutor`, executor disposal
  no-op, and keyed child reconciliation. Module `:adapters:ui-kotlin:test` +
  `:ktlintCheck` are green. The FLUX-040 form adapters share the same test file
  and gate.

### Parity gate — CLEARED (FLUX-076 done)
AGENTS.md: a primitive needs **both** adapter kits before it is advertised. iOS
(`adapters/ui-swift`) now has the `Gesture` adapter (FLUX-076 landed), so the
FLUX-041 gesture primitive satisfies the parity rule and may be seeded into the
public surface / `prelude.flux`.

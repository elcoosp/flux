---
id: FLUX-041
status: todo
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

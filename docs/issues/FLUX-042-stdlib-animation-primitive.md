---
id: FLUX-042
status: partial
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
  - motion
source: CHANGELOG.md §PRD-N (deferred: "the signal-graph animation primitive") + roadmap §4
related_adrs:
  - ADR-0047
  - ADR-0045
---

# FLUX-042: Signal-graph animation primitive

> **Status note (2026-08-29):** relabeled `done` → `partial`. A source grep of
> `stdlib/` finds **no animation primitive**, and neither host reconciler branches
> on animation (grep of `ShadowTreeReconciler.swift` / `DirtyReconciler.kt` for
> `Animation` returns nothing). `flux-codegen-core` mentions it only in template/
> comment scaffolding. A Flux app cannot yet author signal-driven animation.

- **Lane:** LANE-N (Phase 2)
- **Depends on:** ADR-0048 convergence decision (LANE-J) — animation targets the
  host's native animation API
- **Source:** `CHANGELOG.md` §PRD-N deferred + roadmap §4
- **Related ADRs:** ADR-0047, ADR-0045

## Problem Statement

An animation primitive tied into the signal graph (spring/timing curves driving
signals, not just discrete patches) is deferred. This is where "10x DX and
incredible performance" is judged hardest against SwiftUI/Compose native animation.

## Solution

An `Animate(signal, curve)` primitive that drives a signal through a spring/timing
curve; the host maps it to `withAnimation`/`AnimationSpec` natively (codegen emits
the real native API). Pairs with async capabilities (ADR-0045) for sequenced
animation.

## Implementation Decisions

- The curve is data the host consumes; the primitive does not ship animation frames
  on the wire.
- Build after the convergence decision so the host animation API is settled.

## Testing Decisions

- Parity trace test: an `Animate` node maps to the expected native animation call
  on both backends.

## Out of Scope

- Form validation (FLUX-040), gestures (FLUX-041).

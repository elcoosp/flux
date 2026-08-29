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

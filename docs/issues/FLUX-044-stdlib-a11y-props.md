---
id: FLUX-044
status: done
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - a11y
source: CHANGELOG.md §PRD-N (deferred: "a11y props") + roadmap §4
related_adrs:
  - ADR-0047
---

# FLUX-044: Accessibility props threaded through the adapter contract

- **Lane:** LANE-N (Phase 2)
- **Depends on:** FLUX-037..043 (each primitive gains a11y as it lands)
- **Source:** `CHANGELOG.md` §PRD-N deferred + roadmap §4
- **Related ADRs:** ADR-0047 (adapter contract is the single place)

## Problem Statement

Accessibility props (labels, roles, focus order) threaded through the adapter
contract from day one of each new primitive are deferred. Retrofitting a11y after
40 components ship is far more expensive.

## Solution

Add a11y props (`label`/`role`/`focusOrder`) to the adapter contract (Appendix F)
on both `adapters/ui-kotlin` and `adapters/ui-swift`, and require every new
primitive (FLUX-037..043) to thread them. The "one platform lowering per component"
rule (AGENTS.md §3.5) keeps it a single edit, not two.

## Implementation Decisions

- a11y is part of the same per-primitive PR as FLUX-037..043 (a checklist item, not
  a separate landing per component).
- No new wire field; a11y is a host rendering concern.

## Testing Decisions

- Each new primitive's parity test asserts the a11y prop reaches the native view's
  accessibility element on both backends.

## Out of Scope

- The iOS convergence port itself (ADR-0048 / LANE-J).

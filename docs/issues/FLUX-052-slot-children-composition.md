---
id: FLUX-052
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - PRD-N
  - PRD-L
labels:
  - language
  - grammar
  - adr
source: CHANGELOG.md §PRD-S (deferred: "slot/children composition for containers like Modal")
related_adrs:
  - ADR-0035
---

# FLUX-052: Slot/children composition for containers (ADR-gated)

- **Lane:** LANE-L (Phase 1)
- **Depends on:** PRD-N (containers exist), PRD-L (grammar frozen)
- **Source:** `CHANGELOG.md` §PRD-S deferred
- **Related ADRs:** ADR-0035 template

## Problem Statement

Containers like `Modal`/`Column`/`Row` need a children/slot composition model so a
component can take child content. PRD-S deferred it as ADR-gated.

## Solution

ADR for the children/slot model, then grammar + type + lower support, then parity
tests. Pairs with FLUX-038 (`Modal`) which needs children.

## Implementation Decisions

- Targets the frozen indentation grammar.
- Children are a typed slot, not a string blob.

## Testing Decisions

- A container with child components lowers + type-checks; parity trace pins dev/release.

## Out of Scope

- The concrete `Modal` primitive (FLUX-038) — this is the language feature it uses.

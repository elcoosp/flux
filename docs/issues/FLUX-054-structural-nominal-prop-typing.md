---
id: FLUX-054
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - PRD-L
labels:
  - language
  - types
  - adr
source: CHANGELOG.md §PRD-S (deferred: "structural vs nominal typing for props")
related_adrs:
  - ADR-0035
---

# FLUX-054: Structural vs nominal prop typing (ADR-gated)

- **Lane:** LANE-L (Phase 1)
- **Depends on:** PRD-L
- **Source:** `CHANGELOG.md` §PRD-S deferred
- **Related ADRs:** ADR-0035 template

## Problem Statement

Roadmap §3 names "structural vs nominal typing for props" as a type-system gap.
PRD-S deferred it as ADR-gated.

## Solution

ADR for the prop-typing model, then extend `flux-types` to support structural prop
records where useful, then parity tests.

## Implementation Decisions

- Must not break the ADR-0047 codegen registry's prop contract (§3.2 derived
  indices) — prop shapes stay name-derived.
- Targets the frozen grammar.

## Testing Decisions

- A structurally-typed prop passed to a component type-checks; parity trace pins
  dev/release.

## Out of Scope

- In-language Result/error propagation (FLUX-055).

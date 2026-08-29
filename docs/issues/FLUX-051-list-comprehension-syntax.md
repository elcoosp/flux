---
id: FLUX-051
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - PRD-L
labels:
  - language
  - grammar
  - adr
source: CHANGELOG.md §PRD-S (deferred: "list-comprehension / iteration syntax")
related_adrs:
  - ADR-0035
  - ADR-0037
---

# FLUX-051: List-comprehension / iteration syntax (ADR-gated)

- **Lane:** LANE-L (Phase 1)
- **Depends on:** PRD-L (grammar frozen)
- **Source:** `CHANGELOG.md` §PRD-S deferred
- **Related ADRs:** ADR-0035/0037 template (each gap → ADR → production → close)

## Problem Statement

Rendering a list of items needs iteration syntax; today there is no
list-comprehension / `for` construct. PRD-S deferred it as ADR-gated (not safe to
land without risking the grammar freeze PRD-L established).

## Solution

File an ADR (next free ADR number) for the iteration syntax, land the grammar
production in `flux-parser`, extend the type checker + lower to IR, and add a
`flux-parity` trace test. Mirror the ADR-0035/0037 template: ADR → production →
close.

## Implementation Decisions

- Must target the frozen ADR-0029 indentation grammar — no brace syntax.
- Each step is its own commit (ADR, then grammar, then type/lower).

## Testing Decisions

- A `for`/comprehension fixture lowers + type-checks; parity trace pins dev/release.

## Out of Scope

- Slot/children composition (FLUX-052), nullable chaining (FLUX-053), structural
  typing (FLUX-054), in-language Result (FLUX-055).

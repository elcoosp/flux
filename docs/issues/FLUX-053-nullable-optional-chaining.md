---
id: FLUX-053
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - PRD-L
labels:
  - language
  - types
  - adr
source: CHANGELOG.md §PRD-S (deferred: "nullable/optional chaining ergonomics")
related_adrs:
  - ADR-0035
---

# FLUX-053: Nullable / optional chaining ergonomics (ADR-gated)

- **Lane:** LANE-L (Phase 1)
- **Depends on:** PRD-L
- **Source:** `CHANGELOG.md` §PRD-S deferred
- **Related ADRs:** ADR-0035 template

## Problem Statement

Real apps need nullable values + safe chaining (`?.`). PRD-S deferred it as
ADR-gated (a type-system gap for real apps, roadmap §3).

## Solution

ADR for the nullability model, then extend `flux-types` (a `Null`/`Option` value
already exists in the VM (`FluxValue::Null`)) + the `?.` lowering, then parity
tests.

## Implementation Decisions

- Reuses `FluxValue::Null` so the runtime contract is already there; this is the
  language/type surface.
- Targets the frozen grammar.

## Testing Decisions

- A `?.` chain over a `Null` value lowers + type-checks; parity trace pins dev/release.

## Out of Scope

- Structural-vs-nominal prop typing (FLUX-054).

---
id: FLUX-033
status: done
lane: LANE-R
phase: "Phase 8"
blocked_by:
  - FLUX-030
  - PRD-K
labels:
  - docs
  - guide
  - errors
source: CHANGELOG.md §PRD-R (deferred: "the ...troubleshooting guide set") + roadmap §10
related_adrs:
  - ADR-0045
---

# FLUX-033: Troubleshooting guide keyed to the FluxError taxonomy

- **Lane:** LANE-R (Phase 8)
- **Depends on:** FLUX-030, PRD-K (`FluxError` taxonomy)
- **Source:** `CHANGELOG.md` §PRD-R deferred + roadmap §10

## Problem Statement

Roadmap §10 wants a troubleshooting guide keyed to the `FluxError` taxonomy from
Phase 0.3. Without it, every error category (Parse/Type/Wire/Vm/Codegen/Capability/
Runtime) needs a human explanation + fix path.

## Solution

A troubleshooting guide with one section per `FluxError` variant, each showing the
typical message shape, the root cause, and the fix — generated/derived from the
error taxonomy so it can't drift.

## Implementation Decisions

- Keyed off `flux-types`' `FluxError` enum; CI asserts every variant has a guide
  section.

## Testing Decisions

- CI link/coverage check: every `FluxError` variant is documented.

## Out of Scope

- The on-device overlay (FLUX-028) — runtime UX, not docs.

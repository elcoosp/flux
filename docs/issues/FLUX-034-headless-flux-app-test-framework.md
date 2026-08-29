---
id: FLUX-034
status: todo
lane: LANE-R
phase: "Phase 8"
blocked_by:
  - FLUX-023
labels:
  - dx
  - test
  - ecosystem
source: CHANGELOG.md §PRD-R (deferred: "the headless .flux app testing framework (reusing flux-parity)")
related_adrs: []
---

# FLUX-034: Headless `.flux` app testing framework (reusing flux-parity)

- **Lane:** LANE-R (Phase 8)
- **Depends on:** FLUX-023 (dev/release parity harness)
- **Source:** `CHANGELOG.md` §PRD-R deferred + roadmap §9

## Problem Statement

Roadmap §9 wants "a testing framework for `.flux` apps: component-level tests that
run against the dev VM headlessly." There is a parity harness (`flux-parity`) but
no user-facing app-test API.

## Solution

Expose a user-facing test API on top of `flux-parity`: render a component against
the reference VM headlessly, assert on its shadow tree / emitted signals. Ship as
`flux test` (or a `#[flux_test]` harness) reusing `flux-parity`'s trace diffing.

## Implementation Decisions

- Reuses `flux-parity` (no new VM); the dev VM is the oracle.
- Assertions read the shadow tree / signal graph, not pixels.

## Testing Decisions

- A sample `.flux` component test asserts a signal updates after a synthetic tap.

## Out of Scope

- The package registry (already shipped in PRD-R), crash reporting (FLUX-035).

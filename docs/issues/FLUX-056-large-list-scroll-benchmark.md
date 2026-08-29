---
id: FLUX-056
status: todo
lane: LANE-H
phase: "Phase 5"
blocked_by:
  - PRD-N
  - PRD-J
  - PRD-M
labels:
  - perf
  - benchmark
source: CHANGELOG.md §PRD-T (deferred: "the large-list scroll benchmark requires PRD-N's ScrollView (blocked_by: [PRD-J, PRD-M, PRD-N])")
related_adrs:
  - ADR-0047
---

# FLUX-056: Large-list scroll benchmark (depends on ScrollView + perf harness)

- **Lane:** LANE-H (Phase 5)
- **Depends on:** PRD-N (`ScrollView`), PRD-J (perf harness), PRD-M (CI hardening)
- **Source:** `CHANGELOG.md` §PRD-T deferred
- **Related ADRs:** ADR-0047

## Problem Statement

PRD-T deferred "the large-list scroll benchmark [which] requires PRD-N's
`ScrollView`" (`blocked_by: [PRD-J, PRD-M, PRD-N]`). The §3.10 large-list scroll
budget is unverified without it.

## Solution

Once `ScrollView` (PRD-N) lands, add a 1k/10k-item virtualized scroll benchmark to
`flux-perf-harness` feeding the §3.10 scroll budget gate, run in CI.

## Implementation Decisions

- Reuses `flux-perf-harness`'s `MetricRecord` schema (PRD-J) so it shares the CI
  gate.
- Measures scroll-frame latency + reconciliation ratio on the virtualized list.

## Testing Decisions

- The bench runs in `perf-harness.yml` and fails CI if the scroll budget regresses.

## Out of Scope

- The RN/Flutter published comparison (FLUX-057).

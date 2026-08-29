---
id: FLUX-032
status: todo
lane: LANE-R
phase: "Phase 8"
blocked_by:
  - FLUX-030
labels:
  - docs
  - guide
source: CHANGELOG.md §PRD-R (deferred: "migration guides *from* RN and Flutter")
related_adrs: []
---

# FLUX-032: Migration guides from RN and Flutter

- **Lane:** LANE-R (Phase 8)
- **Depends on:** FLUX-030
- **Source:** `CHANGELOG.md` §PRD-R deferred follow-ups

## Problem Statement

Roadmap §10 names "migration guides *from* RN and Flutter (name the differences
honestly — this is a credibility move)." None exist.

## Solution

Two honest migration guides mapping RN/Flutter concepts → Flux primitives, calling
out what Flux does NOT yet cover (per the roadmap's "90% of use cases" gap).

## Implementation Decisions

- Diff against the real stdlib surface (FLUX-037..044), not aspirational.
- Link each RN/Flutter concept to the Flux primitive or to its open issue.

## Testing Decisions

- Link-check clean; each mapped concept references a real primitive or issue id.

## Out of Scope

- The cookbook (FLUX-031), troubleshooting (FLUX-033).

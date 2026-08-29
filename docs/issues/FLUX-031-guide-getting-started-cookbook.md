---
id: FLUX-031
status: done
lane: LANE-R
phase: "Phase 8"
blocked_by:
  - FLUX-030
labels:
  - docs
  - guide
source: CHANGELOG.md §PRD-R (deferred: "the getting-started/cookbook/migration/troubleshooting guide set")
related_adrs: []
---

# FLUX-031: Getting-started + cookbook guide set

- **Lane:** LANE-R (Phase 8)
- **Depends on:** FLUX-030 (website base)
- **Source:** `CHANGELOG.md` §PRD-R deferred follow-ups

## Problem Statement

The roadmap (§10) calls for "getting started, cookbook per new stdlib primitive,
migration guides from RN and Flutter" — all deferred. A new user cannot onboard
from the current docs alone.

## Solution

Author a getting-started guide (scaffold → `flux dev` → first component → hot
reload) and a per-primitive cookbook that grows as stdlib expands (FLUX-037..044).
Cookbook entries are generated/linked from `flux doc` stdlib schema where possible.

## Implementation Decisions

- One cookbook page per stdlib primitive, cross-linked to its `flux doc` schema.
- Getting-started uses the real `counter`/`router` examples.

## Testing Decisions

- Docs build clean; every stdlib primitive listed in `flux doc` has a cookbook
  page (CI link-check).

## Out of Scope

- Migration/troubleshooting (FLUX-032/033).

---
id: FLUX-092
status: done
lane: LANE-DOCS
phase: "Phase 3"
blocked_by: []
labels:
  - docs
  - website
  - ci
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.9 (make check-i18n-drift.ts a required, not advisory, CI check).
related_adrs: []
---

# FLUX-092: Promote `check-i18n-drift.ts` to a required CI check

- **Lane:** LANE-DOCS (Phase 3 — docs/website)
- **Owner:** Website / docs
- **Source:** plan §2.9
- **Disjoint from:** every other issue.

## Problem Statement

The website has heavy i18n duplication (`es/`, `fr/` content trees) and
`check-i18n-drift.ts` already exists to detect English-source vs translation drift.
The plan (§2.9) notes it is currently **advisory only** — translated docs can rot
silently behind the English source. There's no enforced gate.

## Solution

- Wire `check-i18n-drift.ts` into the website CI as a **required** (gating) check, not
  advisory. A drift beyond an accepted threshold fails the build.
- If the script is incomplete or the threshold is unconfigured, finish it first; this
  issue includes making the check actually enforceable (exit code + CI step).

## Implementation Decisions

- Keep it non-blocking for a short transition window if a large existing backlog of
  drift exists, but flip to blocking within this issue (don't leave it advisory).
- `pnpm-lock.yaml` (5641 lines) is large but out of scope — this is about doc drift,
  not lockfile size.

## Testing Decisions

- A deliberately-drifted translation fixture fails the CI step; a synced tree passes.

## Out of Scope

- Writing the missing translations (that's content work, not this gate).
- The DevTools nightly canary (FLUX-090).

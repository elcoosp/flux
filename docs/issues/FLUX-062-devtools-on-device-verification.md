---
id: FLUX-062
status: todo
lane: LANE-P
phase: "Phase 4"
blocked_by:
  - FLUX-058
  - FLUX-059
  - FLUX-060
  - FLUX-061
labels:
  - devtools
  - verification
  - ios
  - android
source: CHANGELOG.md §PRD-P (deferred: "the 'demoed against a real running app' evidence ... on-device verification")
related_adrs:
  - ADR-0040
---

# FLUX-062: DevTools on-device verification (ship it, not scaffold it)

- **Lane:** LANE-P (Phase 4)
- **Depends on:** FLUX-058/059/060/061 (the views) + a real running app
- **Source:** `CHANGELOG.md` §PRD-P deferred + roadmap §6 ("ship it, not scaffold it")
- **Related ADRs:** ADR-0040

## Problem Statement

PRD-P deferred "the 'demoed against a real running app' evidence the roadmap's
'ship it, not scaffold it' bar requires (on-device verification)." The DevTools
skeleton has never been validated against a real app on a device/sim.

## Solution

Connect `flux-devtools-ui` to a real running host (iOS sim + Android, via the
FLUX-036 showcase apps or `counter`/`router`), exercise every view (component
tree, signal graph, timeline, time-travel, network), and capture the evidence
(screenshots / recorded sessions) proving each is demoed, not just compiled.

## Implementation Decisions

- Uses the real `:7333` DevTools endpoint (ADR-0039/0040) against a real app.
- This is verification evidence, not new code — but any gaps found block the view's
  "done" claim.

## Testing Decisions

- A CI/dev script boots a host + DevTools and asserts each view renders live data
  (the on-device check the roadmap requires).

## Out of Scope

- The views themselves (FLUX-058..061) — this proves them.

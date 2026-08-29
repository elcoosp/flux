---
id: FLUX-043
status: done
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - theming
source: CHANGELOG.md §PRD-N (deferred: "design-token theming") + roadmap §4
related_adrs:
  - ADR-0047
---

# FLUX-043: Design-token theming (codegen into SwiftUI/Compose theme)

- **Lane:** LANE-N (Phase 2)
- **Depends on:** FLUX-037 (layout) + FLUX-039 (Image/Icon)
- **Source:** `CHANGELOG.md` §PRD-N deferred + roadmap §4
- **Related ADRs:** ADR-0047

## Problem Statement

A design-token system (spacing/color/typography scales) codegen'd into both
SwiftUI and Compose theme mechanisms natively — not hardcoded literals per
component — is deferred.

## Solution

A `theme` primitive / stdlib declaration carrying design tokens; codegen emits
native `Color`/`Font`/`Grid` theme extensions on both backends (not per-component
literals). Components reference tokens by name.

## Implementation Decisions

- Tokens are a codegen concern (ADR-0047 single-source), never hardcoded per
  component.
- The `color`/`font` stdlib declarations already exist as a seed pattern.

## Testing Decisions

- Codegen emits a native theme extension containing every declared token on both
  backends (assert in `flux-codegen-{swift,kotlin}` tests).

## Out of Scope

- a11y props (FLUX-044).

---
id: FLUX-036
status: todo
lane: LANE-R
phase: "Phase 8"
blocked_by:
  - FLUX-023
  - FLUX-030
labels:
  - ecosystem
  - i18n
  - guide
source: CHANGELOG.md §PRD-R (deferred: "published state-management patterns, app-level i18n, and the 2–3 showcase apps")
related_adrs: []
---

# FLUX-036: State-management patterns, app-level i18n, showcase apps

- **Lane:** LANE-R (Phase 8)
- **Depends on:** FLUX-023 (parity), FLUX-030 (website)
- **Source:** `CHANGELOG.md` §PRD-R deferred + roadmap §9/§10

## Problem Statement

Roadmap §9/§10 defer three ecosystem items: published opinionated state-management
patterns (global stores, derived signals, async data fetching), app-level i18n
(string externalization + locale-aware formatting), and 2–3 substantial showcase
apps that double as integration tests.

## Solution

1. **State-management guide**: opinionated patterns over the signal graph (global
   stores, derived signals, async data-fetch via ADR-0045 capabilities).
2. **App-level i18n**: a stdlib/capability concern for string externalization +
   locale-aware formatting (distinct from the `website/` en/es docs i18n).
3. **Showcase apps**: 2–3 real apps (beyond `counter`/`router`) exercising the
   Phase 2 stdlib (FLUX-037..044) end-to-end; they double as living integration
   tests + marketing assets.

## Implementation Decisions

- Showcase apps live under `examples/` and are wired into CI as integration tests.
- App i18n reuses the dev server's string-interning story (§3.8) where relevant.

## Testing Decisions

- Each showcase app builds (release codegen) + runs a smoke test in CI.

## Out of Scope

- The `website/` docs i18n (FLUX-030) — that is docs-site locale, not app locale.

---
id: FLUX-035
status: todo
lane: LANE-R
phase: "Phase 8"
blocked_by:
  - PRD-K
labels:
  - ecosystem
  - release
  - ios
  - android
source: CHANGELOG.md §PRD-R (deferred: "release crash reporting (Swift/Kotlin reporters)") + roadmap §9
related_adrs: []
---

# FLUX-035: Release crash reporting (Swift/Kotlin reporters)

- **Lane:** LANE-R (Phase 8)
- **Depends on:** PRD-K (`FluxError` taxonomy)
- **Source:** `CHANGELOG.md` §PRD-R deferred + roadmap §9

## Problem Statement

Roadmap §9: "Crash reporting / error tracking integration for release builds
(Sentry-equivalent): since release is native codegen, this is 'just' a Swift/Kotlin
crash reporter integration, but it needs a story before 1.0." Absent today.

## Solution

Wire a native crash reporter into both release hosts (Swift + Kotlin) that maps a
release crash back to the generated component/source where possible, feeding the
same `FluxError` shape PRD-K established. Keep it release-only (no dev telemetry
leak — note FLUX-028 is dev-only, this is release-only).

## Implementation Decisions

- Host-native reporter (no webview); respects the `#if DEBUG`/`BuildConfig.DEBUG`
  split so dev telemetry (ADR-0040) and release crash reporting never mix.
- No new wire fields; the reporter is a host concern.

## Testing Decisions

- A forced release-path crash (test build) produces a report carrying the component
  id / source reference.

## Out of Scope

- The dev overlay (FLUX-028), the docs site (FLUX-030).

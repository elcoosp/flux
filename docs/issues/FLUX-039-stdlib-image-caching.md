---
id: FLUX-039
status: partial
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
  - media
source: CHANGELOG.md §PRD-N (deferred: "Image remote caching") + roadmap §4
related_adrs:
  - ADR-0047
---

# FLUX-039: Stdlib media — Image (local + remote with caching)

- **Lane:** LANE-N (Phase 2)
- **Depends on:** none
- **Source:** `CHANGELOG.md` §PRD-N deferred + roadmap §4
- **Related ADRs:** ADR-0047

## Problem Statement

`Image` (local + remote with caching) is deferred — the single most-missing media
primitive after `ScrollView`. No app with images works without it.

## Solution

`Image(src)` mapping to `UIImage`/`AsyncImage` (Swift) and `Coil`/`Glide`
(Android) via the ADR-0047 registry, with a host-side cache (Coil on Android,
`URLCache` on iOS). Local `src` is a bundled asset path; remote `src` is a URL.

## Implementation Decisions

- Caching is a host concern (no new wire field); the primitive only carries the
  `src` prop.
- Pin the dev/release mapping with `flux-parity` like `ScrollView`.

## Testing Decisions

- Parity trace test asserts `Image` maps to the expected native view on both
  backends; a JVM test asserts the cache path is hit on a repeat load.

## Out of Scope

- `Icon` vector theming (subset of FLUX-043 theming).

---
id: FLUX-039
status: done
lane: LANE-N
phase: "Phase 2"
blocked_by: []
labels:
  - stdlib
  - primitive
  - media
  - android
  - ios
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

## What shipped

- **Host cache (both platforms):** a single-flight, LRU/in-memory core keyed by
  resolved asset/remote URL, so a repeat load is served from cache and a list of
  identical images collapses to one fetch.
  - Android: `ImageCache` (OkHttp-backed `OkHttpImageFetcher`) in the pure-JVM
    `:host` module, with 6/6 JVM unit tests (TDD) asserting cache hit on repeat
    load, single-flight coalescing, failure-not-cached, LRU eviction, clear.
  - iOS: `ImageCache` actor backed by a dedicated `URLCache` (disk + memory) +
    single-flight, in `FluxUIKit`; `ImageAdapter` now loads through it instead of
    an ad-hoc `URLSession.shared` task. Unit tests added.
- **Android renderer actually renders `Image`:** `ShadowTreeRenderer` had no
  `image` case, so `Image` silently fell through to an empty container (the
  "I see nothing" bug). Added `RenderImage` — it resolves and fetches the `src`
  through the shared `FluxSession.imageCache`, decodes the bitmap, shows a
  placeholder while loading and a red box on failure (BR-003), and threads the
  cache + asset base URL through every renderer.
- **iOS** `ImageAdapter` reuses the shared `ImageCache.shared`; `FluxHost`
  registers it via `AdapterKit` as before.

## Testing Decisions

- Parity trace test asserts `Image` maps to the expected native view on both
  backends; a JVM test asserts the cache path is hit on a repeat load.

## Out of Scope

- `Icon` vector theming (subset of FLUX-043 theming).

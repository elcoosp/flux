---
id: FLUX-068
status: todo
lane: LANE-G
phase: "Phase 9"
blocked_by:
  - ADR-0050
labels:
  - release
  - distribution
  - ios
  - android
source: CHANGELOG.md §roadmap §0.5 / §11 (flux build invokes toolchain; distribution artifacts) + ADR-0036/0050
related_adrs:
  - ADR-0036
  - ADR-0050
---

# FLUX-068: `flux build` actually invokes native toolchain + distribution artifacts

- **Lane:** LANE-G (Phase 9 — release)
- **Depends on:** ADR-0050 (runtime versioning), LANE-F (packaging)
- **Source:** `CHANGELOG.md` roadmap §0.5 ("Make `flux build` actually invoke
  xcodebuild/gradle") + §11 (freeze contracts) + ADR-0036 (packaging gap)
- **Related ADRs:** ADR-0036, ADR-0050

## Problem Statement

`flux build` detects `xcodebuild`/`gradle` and logs a manual command if absent; it
does not actually invoke them (LANE-E baseline: "EMITS ONLY ... best-effort, not
invoked"). ADR-0036's packaging gap (no AAR/xcframework, no embed guide) is only
partially resolved by ADR-0050. A consumer cannot pull a versioned engine.

## Solution

- `flux build` invokes the native toolchain when present and FAILS on non-zero exit
  (LANE-E), keeping the "log manual command" fallback for local envs without
  toolchains.
- Produce distribution artifacts: `FluxHost.xcframework` (iOS) + `:runtimes:android:host`
  AAR (Android) from LANE-F, pin `PROTOCOL_VERSION` (ADR-0050), and write the "Embed
  Flux in an existing app" guide.

## Implementation Decisions

- Invocation respects AGENTS.md §1.3 (no manifest edit without request) — the build
  wiring is CLI code, the artifacts are a CI/publish step.
- Fail-closed version handshake (ADR-0050) is the host-side check; reuse it.

## Testing Decisions

- `flux build --platform ios` on a Mac produces a compiling app; `flux build --platform
  android` uses `./gradlew`; the published AAR/xcframework build in CI.

## Out of Scope

- The package registry (PRD-R, shipped), crash reporting (FLUX-035).

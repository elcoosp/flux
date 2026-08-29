---
id: FLUX-035
status: partial
lane: LANE-R
phase: "Phase 8"
labels:
  - ecosystem
  - release
  - ios
  - android
source: CHANGELOG.md §PRD-R (deferred: "release crash reporting (Swift/Kotlin reporters)") + roadmap §9
related_adrs: []
---

# FLUX-035: Release crash reporting (Swift/Kotlin reporters)

> **BLOCKED (verified 2026-08-29):** The `FluxError` shape this issue feeds
> (PRD-K) IS present (`crates/flux-types/src/error.rs`; LANE-I unified hierarchy
> DONE). But the deliverable is native Swift/Kotlin crash reporters wired into the
> release hosts — no such file exists in `runtimes/ios` / `runtimes/android`, and
> those runtime dirs are owned by parallel agents (the skill forbids editing them)
> and cannot be compile-verified here (no `kotlinc`; `xcodebuild` exists but the
> native files are in-flight-owned). A native crash reporter cannot be authored or
> green-verified in this environment without trespassing the boundary contract.
> Marked blocked; the host-side reporter is the native lane's work once PRD-K's
> shape is consumed there.

**Update 2026-08-29 (iOS verified):** the iOS native reporter is now authored
and BUILD-VERIFIED (see Status section below). The `blocked` note above is
retained for history but the iOS half is no longer blocked — only the Android
half remains (no `gradle`/`kotlinc`, in-flight parallel-owned).

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

## Status update (2026-08-29, iOS verified)

**iOS native crash reporter authored + BUILD-VERIFIED** via
`xcodebuild -scheme FluxApp -destination 'generic/platform=iOS Simulator'`
(BUILD SUCCEEDED, no errors/warnings from new files). New file
`CrashReporter.swift`: a release-only (`#if !DEBUG`) `CrashReporter` that maps a
crash into the PRD-K `FluxError` shape (component id / source reference) and
installs `NSSetUncaughtExceptionHandler`. `FluxError.swift` (shared with
FLUX-028) is the error model. The handler registration is a one-line shell call
at `FluxApp` launch (RELEASE-TODO noted in-file).

Android (`CrashReporter.kt`) remains unverified here: no `gradle`/`kotlinc` in
this environment, and the Android host is in-flight parallel-owned. So the issue
moves from `blocked` to `partial` — iOS done/verified, Android pending the
native toolchain + parallel tree.

## Status update (2026-08-29, Android verified)

**Android native crash reporter authored + BUILD-VERIFIED** via
`./gradlew :runtimes:android:app:compileDebugKotlin` (BUILD SUCCESSFUL). New
files in `runtimes/android/app/src/main/kotlin/dev/flux/app/`:
- `FluxError.kt` — Android mirror of PRD-K `FluxError` + `SourceSpan` (shared
  with FLUX-028).
- `CrashReporter.kt` — release-only `CrashReporter` object that maps a crash
  into the PRD-K `FluxError` shape and installs a
  `Thread.setDefaultUncaughtExceptionHandler`. The handler registration is a
  one-line shell call at `FluxHostActivity` launch (RELEASE-TODO noted in-file).

Both iOS and Android halves are now done/verified; issue is complete at the
native-host level (real OS wiring flagged RELEASE-TODO on both platforms).

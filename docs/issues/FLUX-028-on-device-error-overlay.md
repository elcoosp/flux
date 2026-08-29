---
id: FLUX-028
status: partial
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - PRD-K
labels:
  - dx
  - ios
  - android
  - overlay
  - runtime
source: CHANGELOG.md §PRD-O (deferred: "the native on-device error overlay (PRD-K FluxError + Span)")
related_adrs:
  - ADR-0044
  - ADR-0045
---

# FLUX-028: Native on-device error overlay (PRD-K FluxError + Span)

- **Lane:** LANE-O (Phase 3)
- **Depends on:** PRD-K (span-threaded `FluxError` on the wire)
- **Source:** `CHANGELOG.md` §PRD-O deferred follow-ups
- **Related ADRs:** ADR-0044/0045 (async result cells), AGENTS.md Appendix E §E.6

## Problem Statement

A dev-mode VM/wire fault shows a red banner *somewhere* but there is no dedicated
native (non-webview) error screen with the highlighted `.flux` source span + a
formatted stack through handler dispatch (PRD-O user story 7 & 8). This is the
single most-loved Metro/Flutter DX feature and currently has zero equivalent.

## Solution

A native host screen on both platforms that, on a `FluxError` (VM/Wire/Runtime
variant) in dev mode, renders:
1. the error `message` (what/why/how from PRD-K),
2. the `.flux` source span **highlighted** (map `Span` → file:line via the
   existing `SourceMap`/`DevToolsRouter` span plumbing),
3. a formatted stack through handler dispatch (reuse the telemetry `call_sites`).

Per AGENTS.md Appendix E §E.6 it is a **native** screen (UIKit `UIView` /
Compose `Composable`), never a webview, and never a crash.

## Implementation Decisions

- Consumes PRD-K's `FluxError` + `Span` exactly as the DevTools `SourceMap` does —
  one error shape across host + DevTools (PRD-O user story 8).
- Guarded by `#if DEBUG` / `BuildConfig.DEBUG` (like the telemetry instrumentation)
  so there is zero release impact.
- No new wire fields beyond PRD-K's span-bearing error field.

## Testing Decisions

- iOS (`FluxHostTests`): inject a `FluxError` with a known `Span`, assert the
  overlay view renders the message + highlighted range + non-empty stack.
- Android (`:host` JVM test): same assertion on a `Composable` preview/test.
- Both reuse the PRD-P `SourceMap` span-resolution tests as the shared core.

## Status update (2026-08-29, iOS verified)

**iOS native overlay authored + BUILD-VERIFIED** via
`xcodebuild -scheme FluxApp -destination 'generic/platform=iOS Simulator'`
(BUILD SUCCEEDED, no errors/warnings from new files). New files:
- `FluxError.swift` — iOS mirror of PRD-K `FluxError` + `SourceSpan` (consumed
  by host + DevTools, one error shape).
- `ErrorOverlay.swift` — native `ErrorOverlayView` (UIKit `UIView`, `#if DEBUG`
  guarded, never a webview) rendering message + highlighted span + dispatch
  stack, plus `presentFluxError(_:fileResolver:)` (marked `@MainActor`).

Android (`Composable` overlay) remains unverified here: no `gradle`/`kotlinc`
and the Android host is in-flight parallel-owned.

## Out of Scope

- Crash reporting for release builds (FLUX-035) — different concern.
- The LSP server (FLUX-024) — editor-side, not on-device.

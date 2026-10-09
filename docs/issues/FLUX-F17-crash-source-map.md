# FLUX-F17 — Crash reporter source-map resolution

**Status:** open (follow-up from F12)
**Severity:** low (reporting quality, not a defect)

## Summary

Both `CrashReporter` implementations (`runtimes/ios/FluxHost/Sources/FluxHost/CrashReporter.swift`
and `runtimes/android/app/src/main/kotlin/dev/flux/app/CrashReporter.kt`) install the
platform's uncaught-exception handler and record the raw stack into `lastCrash`
as a `FluxError` with `callSites`. Neither resolves the *top* frame back to a
generated component id / source excerpt via the embedded source map.

The previous inline `RELEASE-TODO` comments (banned by AGENTS.md §3.1) implied
the handlers were unwired; both `install()` functions exist and are called from
the app shell (`FluxApp.kt:14`). The remaining gap is the *resolution* step.

## What a fix needs

* Embed (or make reachable from the app) the per-build source map produced by
  the codegen pipeline (see ADR-0057's `SourceExcerpt` and the codegen-emitted
  metadata).
* On crash, walk the top frames of `callStackSymbols` (iOS) /
  `stackTraceToString` (Android), match each against the codegen's mapping of
  generated symbols → component ids, and populate `FluxError.span` +
  `FluxError.excerpt` before reporting.
* Test the resolution path with a synthetic stack so the mapping is pinned.

## Notes

This is a release-mode-only feature; no dev-tier behavior changes.

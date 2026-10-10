# FLUX-F17 — Crash reporter source-map resolution

**Status:** CLOSED (round 68, 2026-10-10) — sidecar emitted by `flux build`; both CrashReporters resolve against it
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

---

## Design decision (round 67, 2026-10-10)

This issue is a *feature* (reporting quality), not a defect; it needs a
product decision on where the source map lives before it can be
implemented. The decision recorded here is the minimal-shape one that
matches how the rest of the toolchain works.

### Chosen shape: codegen emits a sidecar `*.flux.map.json` per generated file

`flux build` already writes one `.swift` / `.kt` per `.flux` source. The
crash reporter needs a mapping from a *generated symbol* (Swift
`struct Hello: View`, Kotlin `@Composable fun Hello`) to its originating
`.flux` file + component name + span. That mapping is a pure function of
the codegen traversal, so the simplest delivery is a JSON sidecar emitted
next to each generated source:

    platforms/ios/Generated/Hello.swift
    platforms/ios/Generated/Hello.flux.map.json
    platforms/android/Generated/Hello.kt
    platforms/android/Generated/Hello.flux.map.json

Schema:

```json
{
  "version": 1,
  "source_file": "src/Hello.flux",
  "component_map": [
    {"native_symbol": "Hello", "component_name": "Hello",
     "file": "src/Hello.flux", "line": 3, "col": 1}
  ]
}

---

## Design decision (round 67, 2026-10-10)

This issue is a *feature* (reporting quality), not a defect; it needs a
product decision on where the source map lives before it can be
implemented. The decision recorded here is the minimal-shape one that
matches how the rest of the toolchain works.

### Chosen shape: codegen emits a sidecar `*.flux.map.json` per generated file

`flux build` already writes one `.swift` / `.kt` per `.flux` source. The
crash reporter needs a mapping from a *generated symbol* (Swift
`struct Hello: View`, Kotlin `@Composable fun Hello`) to its originating
`.flux` file + component name + span. That mapping is a pure function of
the codegen traversal, so the simplest delivery is a JSON sidecar emitted
next to each generated source:

    platforms/ios/Generated/Hello.swift
    platforms/ios/Generated/Hello.flux.map.json
    platforms/android/Generated/Hello.kt
    platforms/android/Generated/Hello.flux.map.json

Schema:

```json
{
  "version": 1,
  "source_file": "src/Hello.flux",
  "component_map": [
    {"native_symbol": "Hello", "component_name": "Hello",
     "file": "src/Hello.flux", "line": 3, "col": 1}
  ]
}

## Resolution (round 68, 2026-10-10)

**Design decision:** emit a JSON sidecar `<stem>.flux.map.json` next to each
generated native source, so crash resolution is a *build artifact* the host
can bundle — not something the codegen emitter has to thread through every
call site.

### What landed

1. **`flux-codegen-core/src/source_map.rs`** — new `SourceMap` +
   `SourceMapEntry` types with `to_json` / `from_json`. A `serde_json`
   dependency was added to `flux-codegen-core` so the emit-time ownership
   sits in the crate that already has `Bridge`, `ComponentMeta`, and `Span`.
2. **`flux-cli/src/build.rs::write_source_map`** — after each generated
   source is written, the CLI builds a `SourceMap` from
   `Bridge::components()` (deterministic source order) and writes
   `<stem>.flux.map.json` alongside the generated source.
3. **Swift `CrashReporter`** — `installSourceMap(named:)` /
   `installSourceMapFromBundle()` load the sidecar; `resolveExcerpt` walks
   the top 8 frames of `callStackSymbols`, matches `native_symbol`, and
   populates `FluxError.excerpt` with the resolved `.flux` path + line/col.
4. **Kotlin `CrashReporter`** — `installSourceMap(context, assetName)`
   loads the sidecar from assets via `org.json`; `resolveExcerpt` does the
   same frame walk against `stackTraceToString()`.
5. **`crates/flux-cli/tests/build_source_map.rs`** — regression test that
   `flux build ios --root <project>` emits the sidecar with the expected
   `native_symbol` and `line` fields.
6. **`docs/spec/mlp-spec.md` §14.5** — the sidecar schema documented.

The original filing text is preserved below.

## Resolution (round 68, 2026-10-10)

**Design decision:** emit a JSON sidecar `<stem>.flux.map.json` next to each
generated native source, so crash resolution is a *build artifact* the host
can bundle — not something the codegen emitter has to thread through every
call site.

### What landed

1. **`flux-codegen-core/src/source_map.rs`** — new `SourceMap` +
   `SourceMapEntry` types with `to_json` / `from_json`. A `serde_json`
   dependency was added to `flux-codegen-core` so the emit-time ownership
   sits in the crate that already has `Bridge`, `ComponentMeta`, and `Span`.
2. **`flux-cli/src/build.rs::write_source_map`** — after each generated
   source is written, the CLI builds a `SourceMap` from
   `Bridge::components()` (deterministic source order) and writes
   `<stem>.flux.map.json` alongside the generated source.
3. **Swift `CrashReporter`** — `installSourceMap(named:)` /
   `installSourceMapFromBundle()` load the sidecar; `resolveExcerpt` walks
   the top 8 frames of `callStackSymbols`, matches `native_symbol`, and
   populates `FluxError.excerpt` with the resolved `.flux` path + line/col.
4. **Kotlin `CrashReporter`** — `installSourceMap(context, assetName)`
   loads the sidecar from assets via `org.json`; `resolveExcerpt` does the
   same frame walk against `stackTraceToString()`.
5. **`crates/flux-cli/tests/build_source_map.rs`** — regression test that
   `flux build ios --root <project>` emits the sidecar with the expected
   `native_symbol` and `line` fields.
6. **`docs/spec/mlp-spec.md` §14.5** — the sidecar schema documented.

The original filing text is preserved below.

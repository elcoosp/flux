---
id: FLUX-075
status: in-progress
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - FLUX-028
  - PRD-K
labels:
  - dx
  - ios
  - android
  - overlay
  - runtime
  - wire
related_adrs:
  - ADR-0057
  - ADR-0025
  - FLUX-050
source: CHANGELOG.md §PRD-O (native on-device error overlay) + cross-host audit
---

# FLUX-075: Unified on-device error overlay (one model, one look, real source)

- **Lane:** LANE-O (Phase 3)
- **Depends on:** FLUX-028 (overlay scaffold exists, both platforms), PRD-K (`FluxError` taxonomy)
- **Related ADRs:** ADR-0057 (wire contract), ADR-0025 (binary frames), FLUX-050 / ADR-0056 (fail-closed versioning)

## Problem Statement

A cross-host audit of `runtimes/android` and `runtimes/ios` shows the two error
overlays are neither the same to look at nor actually informative:

1. **Android has two overlays, only the dumb one is live.** The polished
   `ErrorOverlay(error: FluxError, fileResolver)` Composable (FLUX-028) exists but
   is wired only to `CrashReporter`. The live error path is a separate private
   2-line `ErrorOverlay(message)` banner in `FluxRoot.kt`, fed by
   `executor.onError: (String) -> Unit`. `onError` is only ever handed primitive
   strings like `"vm: TYPE_MISMATCH @42"` / `"wire: ..."` — no file, no line, no
   how-to-fix.
2. **iOS has three overlays, all under-informed.** `ErrorOverlay(VmError)` shows a
   bytecode offset and nothing else. `ServerErrorOverlay(ServerError)` shows a
   message plus a raw `file_id`/byte-offset span never resolved to `path:line:col`.
   `ReconnectingOverlay` is a third, different shape.
3. **No runtime fault maps to `.flux` source.** VM faults carry only a bytecode
   offset. The dev server already computes each handler's `ClosureRef.span`
   (Appendix D §D.7) and ships it, but Android *drops* it in `decodeClosureRef`
   and iOS never turns it into `path:line:col`. Neither host has source text to
   render a snippet/caret.
4. **No shared taxonomy / visual language.** Android `FluxErrorKind{VM,WIRE,
   RUNTIME,CAPABILITY}` (4) vs iOS `VmErrorKind` (8) + `ServerError`. Severity is
   encoded by platform, not by kind: Android = full-screen opaque red; iOS =
   thin-material card.

## Solution (summary)

A single error model, a single visual spec, and one wire change that makes faults
traceable offline.

### 1. One error model — `FluxError`
Both platforms collapse onto `FluxError { kind, message, span?, excerpt?,
callSites? }` with a shared `FluxErrorKind` of eight values
(`PARSE, TYPE, WIRE, VM, RUNTIME, CAPABILITY, COMPILE, SERVER`). iOS folds
`VmError` + `ServerError` into it; Android promotes its existing `FluxError` to
carry the richer `kind` set and the new `excerpt` field. Both render through one
component.

### 2. One visual spec (ADR-0057 §Design)
Severity is a function of `kind`, not the platform:
- **Fatal compile error (`COMPILE`/`SERVER`)** → full-bleed red panel, top-aligned,
  persistent (keeps the last good tree dimly behind a scrim).
- **VM / wire / runtime fault** → dismissible bottom card (top-left on compact),
  last good tree stays visible (Appendix E §E.6).
- **Reconnect** → amber info card, distinct from red faults.
Both: title = `kind`, message = what/why/how, `path:line:col`, optional source
snippet with a `^` caret, optional "how to fix" hint line. Material token parity
on both platforms.

### 3. Wire keystone — `SourceExcerpt` (ADR-0057)
Add a server-computed `SourceExcerpt { file_id, byte_start, byte_end, line, col,
snippet }` to:
- each `HandlerDef` (Appendix D §D.8) — so a VM fault maps `offset → handler →
  span + line/col + snippet` offline, and
- the `Error` frame (§D.12.3) — so a compile/type error ships `path:line:col` +
  a snippet directly.
Computed once at compile time from the source text the server already holds;
resolved to `path` at render time via the existing `source_map` (file_id → path)
already in every Init/Delta frame. No host round-trip, works with the socket down.
Protocol version bumped to `2` (fail-closed; old hosts show a red banner, never
mis-decode) per FLUX-050 / ADR-0056 — both decoders rewritten in this change.

## Testing Decisions
- Rust: round-trip test for `HandlerDef` + `Error` frame carrying `SourceExcerpt`
  through `flux-ir-serde` (no decoder drift).
- Android (`:host` JVM): `FluxExecutor` emits a `FluxError` with span + excerpt on
  a synthetic VM fault; `FluxFrame` decodes `SourceExcerpt` and resolves file_id.
- iOS: `FluxExecutor` maps `VmError`/server error to `FluxError`; overlay snapshot
  test asserts `path:line:col` + snippet render.
- Parity: both hosts consume the same `FluxError` shape and the same excerpt bytes.

## Status
In progress — wire + both overlays implemented in this change.

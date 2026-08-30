# ADR-0057: Unified on-device error overlay wire + model contract

- **Status:** accepted
- **Date:** 2026-08-30
- **Decides:** one `FluxError` model, one overlay visual language, and the
  `SourceExcerpt` wire field that makes VM/runtime faults traceable to `.flux`
  source on-device.
- **Supersedes:** the per-platform ad-hoc overlays (Android `FluxRoot` string
  banner + FLUX-028 `ErrorOverlay`; iOS `ErrorOverlay`/`ServerErrorOverlay`/
  `ReconnectingOverlay`).
- **Related:** FLUX-075, FLUX-028, PRD-K, ADR-0025 (binary frames), FLUX-050 /
  ADR-0056 (fail-closed version handshake).

## Context

The two runtime hosts render dev-mode faults through different components, with
different data and different severity encoding. The richest path we designed
(FLUX-028 `FluxError` + `SourceSpan`) is dead code on Android; the live path is a
2-line string. Neither host resolves a file id to a path, and neither has source
text to show a snippet. A VM runtime fault is therefore surfaced as "vm:
TYPE_MISMATCH @42" with no source — the opposite of the Metro/Flutter DX we are
trying to beat.

The dev server already computes each handler's `ClosureRef.span` (Appendix D
§D.7) and ships a `ServerError` span (§D.12.3), but:
- Android's `decodeClosureRef` discards the span (`r.u32() x3 // span ignored`);
- iOS decodes the closure span but never maps it to `path:line:col`;
- neither host carries source text, so a caret/snippet is impossible client-side.

## Decision

### 1. Single error model
Adopt one `FluxError` shape on both hosts:

```
FluxError {
  kind:     FluxErrorKind   // PARSE | TYPE | WIRE | VM | RUNTIME | CAPABILITY | COMPILE | SERVER
  message:  String          // what / why / how (authored server-side, PRD-K §3.11)
  span?:    SourceSpan      // file_id, line, col  (derived from the wire excerpt)
  excerpt?: SourceExcerpt   // line text + caret position, server-computed
  callSites: [String]       // formatted dispatch stack (telemetry call_sites)
}
```

iOS folds `VmError` (8 kinds) and `ServerError` into this; the 8 VM kinds are
preserved inside `FluxErrorKind.VM` via a sub-kind string on the message so no
diagnostic detail is lost. Android keeps its existing `FluxError` and widens
`FluxErrorKind` to the eight-value set. Both render through one component.

### 2. Single visual language
Severity is a property of `kind`, identical on both platforms:

| Kind | Treatment | Footprint |
|---|---|---|
| `COMPILE` / `SERVER` | full-bleed red panel, top-aligned, persistent | scrim over last good tree |
| `VM` / `WIRE` / `RUNTIME` / `CAPABILITY` | dismissible bottom card | last good tree stays visible (§E.6) |
| reconnect (not a `FluxError`) | amber info card | distinct from red |

Both cards share: title (`kind`), message, `path:line:col`, optional source
snippet with a `^` caret, optional "how to fix" hint. Material token parity (same
corner radius, same spacing, same red/amber) so a developer sees one product.

### 3. Wire keystone — `SourceExcerpt`
Add a server-computed `SourceExcerpt` (Appendix D §D.7 addendum / §D.12.3 addendum):

```
SourceExcerpt {
  has:      u8          // 1 ⇒ present
  file_id:  u32
  byte_start: u32
  byte_end:   u32
  line:     u16         // 1-based
  col:      u16         // 1-based
  snippet:  u16-len UTF-8   // the offending source line, trimmed
}
```

- **Per `HandlerDef` (§D.8):** appended after the `ClosureRef`, gated by `has`.
  Lets a VM fault map `offset → handler → span + line/col + snippet` with zero
  host round-trip (the host already holds `source_map` for `file_id → path`).
- **On the `Error` frame (§D.12.3):** appended after the existing `span`, gated by
  `has`, carrying the diagnostic's `path:line:col` + snippet.

Computed once at compile time from the source text the server already holds in
`Pipeline::sources`; path resolution happens client-side from the shipped
`source_map`. Offline-safe: a fault with the socket down still shows source.

### 4. Protocol version
Bump `PROTOCOL_VERSION` 1 → 2. The new `HandlerDef`/`Error` layout is not
byte-compatible with v1 decoders; a v1 host reading v2 would desync. FLUX-050 /
ADR-0056 require fail-closed versioning, so an old host refuses with a red banner
rather than mis-decode. All three decoders (Rust round-trip, iOS, Android) are
rewritten together in FLUX-075; no v1 host ships to users yet.

## Consequences

- **Good:** faults are now informative and identical across platforms; offline;
  no host-side source scanning. Server owns what/why/how (PRD-K §3.11).
- **Bad:** frame size grows slightly per handler (one `SourceExcerpt`, a few
  bytes) — bounded by handler count, acceptable. Compile-error frames carry one
  extra excerpt.
- **Migration:** dev server emits v2; both hosts decode v2. v1 hosts fail closed.

## Alternatives considered
- **Host round-trips the server for a snippet on fault.** Rejected: needs the
  socket up, adds latency to the worst moment (a crash), and duplicates the
  DevTools telemetry path.
- **Keep per-platform overlays, just prettify.** Rejected: does not fix the shared
  taxonomy or the source-trace gap; re-creates the drift we are removing.
- **Append excerpt only to `Error` frame, derive VM-fault source via telemetry
  `SourceMap`.** Rejected: VM runtime faults are not compile errors and the
  telemetry `SourceMap` is DevTools-only, never shipped to hosts.

## Follow-ups
- `FluxErrorKind.PARSE`/`TYPE` currently only arrive via the `Error` frame; wire
  the parser/type-checker diagnostic kinds into the message sub-kind.
- Property test: every error path produces a non-empty `span` or `excerpt` when
  source is available (PRD-K user story 4).

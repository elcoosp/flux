---
id: PRD-O
status: open
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - PRD-L
  - PRD-K
labels:
  - epic
  - prd
  - dx
  - lsp
  - editor
  - overlay
  - ios
  - android
source: docs/roadmaps/flux-roadmap-to-1.0.md §1.2,§3,§12,§13
related_adrs:
  - ADR-0029
---

# PRD-O: Editor DX — LSP, VS Code Extension, On-Device Error Overlay

- **Lane:** LANE-O (Phase 3)
- **Depends on:** PRD-L (grammar frozen), PRD-K (span-threading + error taxonomy)
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §1.2, §3, §12, §13
- **Related ADRs:** ADR-0029 (frozen grammar), AGENTS.md §3.11 (diagnostics bar), PRD-K (FluxError)

## Problem Statement

The single highest-leverage "10x DX" investment is completely unstarted: there is no LSP, no editor
extension, no linter, no formatter for `.flux` itself (the formatter is PRD-L). Without inline
diagnostics, go-to-definition, hover types, and autocomplete, Flux cannot deliver on "time-to-first-
error faster than RN/Flutter," and without an on-device error overlay there is zero equivalent of
Metro/Flutter's most-loved DX feature. This PRD covers the editor surface; the formatter (PRD-L) and
the error taxonomy/span-threading (PRD-K) it builds on are separate PRDs.

## Solution

Build `flux-lsp` (new crate): diagnostics-as-you-type reusing PRD-S's diagnostic quality bar, go-to-
definition, hover types, prop/capability autocomplete, rename-refactor. Build a VS Code extension
(syntax highlighting matched to the frozen grammar, LSP client, inline hot-reload status, "run on
device" command). Add `flux doctor` (toolchain/device/wire-version/stdlib check). Surface `flux build`
toolchain invocation as good CLI UX. And ship the on-device error overlay: a native (non-webview)
screen showing the message, the highlighted `.flux` source span, and a formatted stack through handler
dispatch — consuming PRD-K's span-threaded errors.

## User Stories

1. As a Fluff app developer, I want inline `.flux` diagnostics in my editor with the precise span and a
   suggested fix, so that I find errors before running anything.
2. As a Fluff app developer, I want go-to-definition and hover types, so that I can navigate a `.flux`
   codebase as easily as a Rust/Kotlin one.
3. As a Fluff app developer, I want autocomplete for props and capabilities, so that I do not memorize
   the primitive surface.
4. As a Fluff app developer, I want a VS Code extension with syntax highlighting + inline hot-reload
   status + "run on device", so that my whole loop lives in one editor.
5. As a Fluff app developer, I want `flux doctor` to report toolchain/device/wire-version/stdlib status
   in one command, so that environment problems are diagnosed fast (like `react-native doctor`).
6. As a Fluff app developer, I want a build failure to tell me "your `.flux` is wrong" vs "your
   Xcode/Gradle setup is wrong", so that I debug the right layer.
7. As a Fluff app developer, I want an on-device error overlay (native, not webview) showing the
   message + highlighted source span + stack, so that a dev-mode runtime error is never a blank screen
   or a crash (AGENTS.md Appendix E §E.6).
8. As a Flux core engineer, I want the on-device overlay to consume PRD-K's span-threaded `FluxError`,
   so that the overlay and DevTools share one error shape.

## Implementation Decisions

- **LSP reuses the compiler:** `flux-lsp` is a thin server over `flux-parser`/`flux-types`/
  `flux-ir`; it does not re-implement analysis. Diagnostics produced here must match PRD-S's rustc-grade
  quality bar so the LSP and the CLI/DevTools never disagree on a diagnostic.
- **Frozen grammar only:** the VS Code syntax highlighter targets the ADR-0029 indentation grammar
  exclusively (PRD-L guarantees brace syntax is gone from CI). Building against a frozen grammar is the
  whole point of sequencing PRD-L first.
- **Overlay is native, not webview:** per AGENTS.md Appendix E §E.6 a VM/wire fault shows a red banner,
  never a crash; the overlay is a native host screen rendering PRD-K's `FluxError` + `Span`.
- **`flux doctor` shape:** one command reporting (a) toolchain versions, (b) device/simulator
  availability, (c) wire-protocol version match between dev server and connected hosts (reuses PRD-M's
  version story), (d) stdlib parse-check status. It is a CLI command alongside `init/dev/build/doc/fmt`.
- **No new wire fields** in this PRD beyond PRD-K's span-bearing error field; the overlay reads what
  PRD-K puts on the wire.

## Testing Decisions

- **Good test:** LSP integration tests asserting that editing a fixture produces the expected diagnostic
  at the expected span, and that go-to-definition jumps to the right declaration; overlay tests asserting
  the native screen renders the PRD-K error + highlighted span. Not tests of LSP transport framing.
- **Modules to test:** `flux-lsp` diagnostic/hover/completion providers, the VS Code extension's LSP
  client wiring, `flux doctor`'s checks, and the on-device overlay renderer.
- **Prior art:** the CLI's existing diagnostic rendering (AGENTS.md §3.11) and PRD-K's error taxonomy are
  the sources to mirror; the DevTools component-tree source jump (PRD-P) shares the span plumbing.

## Out of Scope

- The grammar freeze / formatter (PRD-L).
- The error taxonomy + span-threading (PRD-K) — this PRD consumes them.
- DevTools signal-graph / timeline (PRD-P) — shares spans but is a separate surface.
- New stdlib primitives (PRD-N).

## Further Notes

PRD-O is sequenced after PRD-L and PRD-K because building an LSP against a moving grammar or an
unstable error shape would be rework. It is the concrete deliverable behind roadmap §1.2 "10x DX."

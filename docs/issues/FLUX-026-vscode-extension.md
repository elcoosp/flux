---
id: FLUX-026
status: todo
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - FLUX-024
  - PRD-L
labels:
  - dx
  - lsp
  - editor
  - vscode
source: CHANGELOG.md §PRD-O (deferred: "the VS Code extension (syntax highlight + LSP client + hot-reload status + 'run on device')")
related_adrs:
  - ADR-0029
---

# FLUX-026: VS Code extension (syntax highlight + LSP client + hot-reload status + run on device)

- **Lane:** LANE-O (Phase 3)
- **Depends on:** FLUX-024 (`flux-lsp` server), PRD-L (frozen grammar for the highlighter)
- **Source:** `CHANGELOG.md` §PRD-O deferred follow-ups
- **Related ADRs:** ADR-0029 (frozen grammar)

## Problem Statement

There is no editor extension of any kind. The single highest-leverage DX
investment (roadmap §5) is "completely unstarted." Editors have no syntax
highlighting, no inline diagnostics, no hot-reload status, no "run on device."

## Solution

A VS Code extension (`editors/vscode/`, new dir) that:
1. Ships a TextMate/`.tmLanguage` grammar **generated from the frozen ADR-0029
   indentation grammar** (not hand-maintained — regenerate from `flux-parser`'s
   keyword map so it can't drift).
2. Wires an LSP client to `flux-lsp` (stdio) for diagnostics + (later) go-to-def.
3. Shows inline **hot-reload status** (subscribes to the dev server's
   `:7333` telemetry / `:7331` patch channel and renders "saved ✔ / compiling /
   reloaded" in the status bar).
4. Adds a **"Run on device"** command that launches `flux dev --ws-host 0.0.0.0`
   and prints the LAN URL so a physical device can attach.

## Implementation Decisions

- The extension is a thin client; **all analysis stays in `flux-lsp`** (PRD-O).
- Syntax highlighting targets the frozen indentation grammar exclusively — if
  `flux fmt --check` is green, the highlighter matches.
- Hot-reload status reads the dev server's existing WebSocket/telemetry frames
  (ADR-0039/0040), no new wire fields.

## Testing Decisions

- The generated `.tmLanguage` is asserted to round-trip the stdlib's keyword set
  (a CI check: every keyword in `flux-parser/src/lexer.rs` appears in the grammar).
- Extension packaging `vsce ls` / `eslint` clean; a fixture `.flux` file shows the
  expected token scopes.

## Out of Scope

- go-to-def/hover/completion wiring on the client side (depends on FLUX-027).
- The native on-device overlay (FLUX-028) — that is a host runtime screen, not
  editor UI.

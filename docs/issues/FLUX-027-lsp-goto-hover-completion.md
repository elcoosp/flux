---
id: FLUX-027
status: todo
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - FLUX-024
  - FLUX-025
labels:
  - dx
  - lsp
  - editor
source: CHANGELOG.md §PRD-O (deferred: "go-to-definition/hover/autocomplete providers")
related_adrs:
  - ADR-0047
---

# FLUX-027: `flux-lsp` go-to-definition, hover types, prop/capability completion

- **Lane:** LANE-O (Phase 3)
- **Depends on:** FLUX-024 (server), FLUX-025 (type pipeline + symbol data)
- **Source:** `CHANGELOG.md` §PRD-O deferred follow-ups
- **Related ADRs:** ADR-0047 (primitive registry carries the prop/cap surface)

## Problem Statement

PRD-O user stories 2 & 3 (go-to-definition, hover types, prop/capability
autocomplete) are entirely deferred. A Flux codebase is navigable only by grep.

## Solution

Extend `flux-lsp` with three LSP providers over the existing compiler artifacts:
- **Go-to-definition** — from a usage span, resolve to its declaration's `Span`
  (reuse PRD-P's `SourceMap` span→editor-link plumbing; the `flux-lsp` and
  `flux-devtools-ui` source maps share one shape).
- **Hover** — type/declaration summary for the symbol under the cursor, drawn
  from `flux-types` and stdlib prelude docs.
- **Completion** — prop names from the ADR-0047 primitive registry + capability
  names from `flux-types`' capability prelude, with the derived `prop_index_for_name`
  contract (§3.2) respected (suggest, never hardcode indices).

## Implementation Decisions

- Providers are pure functions over `(LoweredIr, TypedAST)` so they are unit-
  testable without a socket (mirror PRD-P's UI-free `SourceMap` testing style).
- Completion uses the registry's authoritative prop/cap names — never a
  hand-maintained list that can desync from codegen.

## Testing Decisions

- `goto_definition(fixture, cursor_at_usage)` returns the declaration `Span`.
- `completion(fixture_at_T`)` includes the expected prop/cap in the returned
  `CompletionItem`s.

## Out of Scope

- Rename-refactor (separate, lower priority — file if needed). The overlay
  (FLUX-028) is a different surface.

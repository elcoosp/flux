---
id: FLUX-029
status: done   # verified: incremental didChange + debounced re-analysis landed (crates/flux-lsp/src/lib.rs did_change; util.rs apply_incremental_change)
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - FLUX-024
labels:
  - dx
  - lsp
  - performance
source: CHANGELOG.md §PRD-O (deferred: full LSP server — incremental document sync)
related_adrs:
  - ADR-0029
---

# FLUX-029: `flux-lsp` incremental `didChange` + debounced re-analysis

- **Lane:** LANE-O (Phase 3)
- **Depends on:** FLUX-024 (async-lsp server)
- **Source:** `CHANGELOG.md` §PRD-O deferred follow-ups (incremental server behaviour)
- **Related ADRs:** ADR-0029

## Problem Statement

The current `flux lsp <file>` re-reads the whole file per invocation. A real
editor session needs incremental `didChange` (full or incremental content sync)
with debounced re-analysis so typing doesn't re-parse the whole project on every
keystroke, and so `publishDiagnostics` lands within the §3.10 budget.

## Solution

`flux-lsp` keeps an in-memory document cache keyed by URI, handles `didOpen`/
`didChange`/`didClose`, debounces re-analysis (~16 ms, matching the dev server's
frame coalescing), and re-publishes diagnostics. Reuses the parser's allocation-
free hot path (FLUX-003) so a single-file change is well inside the parse budget.

## Implementation Decisions

- `async-lsp` gives the `LanguageServer` trait; implement `did_change` with a
  tokio `sleep`/debounce (no new timer dep — `tokio` is already a workspace dep).
- Document store is an `RwLock<HashMap<Uri, String>>` (parking_lot is a workspace
  dep) — single file server, no cross-file project model needed for diagnostics.

## Testing Decisions

- Send `didOpen` then two `didChange`s within the debounce window; assert only one
  `publishDiagnostics` lands after the window, at the correct span.
- A 500-line fixture re-analyzes within the §3.10 parse budget (criterion micro-bench).

## Out of Scope

- Cross-file project-wide analysis (a later issue if needed). Diagnostics stay
  per-open-document.

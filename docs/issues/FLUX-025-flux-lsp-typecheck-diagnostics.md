---
id: FLUX-025
status: todo
lane: LANE-O
phase: "Phase 3"
blocked_by:
  - FLUX-024
labels:
  - dx
  - lsp
  - diagnostics
  - types
source: CHANGELOG.md §PRD-O (deferred: "flux lsp type-checking (needs a flux-types dependency)")
related_adrs:
  - ADR-0047
---

# FLUX-025: `flux-lsp` type-checking diagnostics (next to parse)

- **Lane:** LANE-O (Phase 3)
- **Depends on:** FLUX-024 (`flux-lsp` crate), PRD-S (rustc-grade type diagnostics)
- **Source:** `CHANGELOG.md` §PRD-O deferred follow-ups
- **Related ADRs:** ADR-0047 (codegen registry / type surface)

## Problem Statement

`flux lsp <file>` (and the new `flux-lsp` server) currently only runs
`flux_parser::parse` — type errors are invisible to the editor. PRD-O deferred
"`flux lsp` type-checking (needs a `flux-types` dependency, which is a manifest
request per §1.3)." A developer hits type errors only at `flux dev` runtime, not
inline — the opposite of the "10x DX" goal.

## Solution

`flux-lsp` runs the full `parse → type_check` pipeline (reusing
`flux-types::type_check`, already a path dep) and publishes both parse and type
diagnostics. The `flux lsp` CLI subcommand is extended to run the same pipeline
and emit parse + type diagnostics (the manifest request for the `flux-types`
dep on `flux-cli` is filed in `MANIFEST_REQUESTS.md`).

## Implementation Decisions

- Reuse PRD-S's `TypeError::render` (file:line:col + caret + `hint:` fix) so the
  type diagnostic `message` already carries what/why/how; map its `Span` to an
  LSP `Range`.
- A `flux lsp` CLI flag `--types` (default on) selects whether type-check runs;
  parse-only stays the fast path for huge files.
- Both LSP server and CLI share one `diagnostics_with_types(file) -> Vec<LspDiagnostic>`
  function in `flux-lsp` (the CLI re-exports it like today's `diagnostics`).

## Testing Decisions

- `diagnostics_with_types` on a fixture with a type mismatch asserts a
  type-source diagnostic at the right span with a non-empty `hint`.
- The CLI path `flux lsp bad.flux` prints the type diagnostic JSON (extend the
  existing `diagnostics_*` tests).

## Out of Scope

- Go-to-definition/hover (FLUX-027) — those need the type-checker *symbol table*,
  a follow-up past this issue. This issue is diagnostics only.

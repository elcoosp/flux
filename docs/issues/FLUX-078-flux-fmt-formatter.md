---
id: FLUX-078
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - ADR-0029
labels:
  - cli
  - dx
  - language
  - tooling
source: roadmap §3 (Phase 1) — "Formatter (`flux fmt`) — non-negotiable for a language with an indentation-sensitive grammar; ship before external contributors touch `.flux` files." Also AGENTS.md §3.6 (grammar is the indentation-based lexer).
related_adrs:
  - ADR-0029
---

# FLUX-078: `flux fmt` — a formatter for `.flux` source

## Problem Statement

`.flux` uses an **indentation-sensitive** grammar (ADR-0029: the live lexer
emits `Indent`/`Dedent`/`Newline` layout tokens). With no canonical formatter,
every author and every example/styles PR will diverge on indentation, trailing
whitespace, and layout — fragmenting the ecosystem the moment external
contributors arrive. The roadmap (Phase 1) calls this **non-negotiable** and
explicitly says to ship it *before* external contributors touch `.flux` files.

Today there is **no** `flux fmt` subcommand: `crates/flux-cli/src/main.rs`
parses `init`/`dev`/`build`/`doc` only; `fmt` is not a recognized subcommand.

## Solution

Add a `flux fmt [--check] [<path>...]` subcommand to `flux-cli`:

- Parse each `.flux` file through the existing `flux-parser` pipeline into the
  AST (`crates/flux-parser`), then pretty-print it back to canonical source.
- Canonical rules (minimal v1, enough to stop style drift):
  - 2-space indentation; re-emit `Indent`/`Dedent` layout from the parsed tree
    (never re-derive from whitespace — the AST is the source of truth).
  - One blank line between top-level `compo`/`record`/`screen` declarations.
  - Trim trailing whitespace; single trailing newline at EOF.
  - Stable prop order = source order (do not reorder fields).
- `--check` exits non-zero (no write) when a file would change — for CI.

Do **not** reinvent a parser: the formatter is a printer over the existing AST,
so it stays correct as the grammar grows (ADR-0029 is the only grammar surface).

## Implementation Decisions

- Reuse `flux-parser`'s `Ast`/`Expr`/`Decl` types; add a `crates/flux-fmt` (or a
  `fmt` module in `flux-parser`) that owns the pretty-printer. Keep it free of
  `tokio`/CLI concerns so it is unit-testable.
- Determinism is the whole point: printing the same AST twice must yield
  byte-identical output (round-trip `parse → print → parse` is the test).
- The LSP (`flux-lsp`) can later call the same printer for "format on save"; the
  formatter must be a library the LSP can import, not CLI-only.

## Testing Decisions

- Round-trip property test (`proptest`): for a corpus of valid `.flux` fixtures,
  `parse(print(parse(src))) == parse(src)` (same AST) and
  `print(parse(print(src))) == print(src)` (idempotent).
- Golden snapshots (`insta`) for `counter`/`router`/`todo` after formatting.
- `--check` returns the right exit code on a deliberately-unformatted fixture.

## Out of Scope

- A linter (`flux lint`) — separate concern; not required for 1.0 style stability.
- Auto-fixing semantic issues (unused signals, type errors) — that is the LSP's
  job, not the formatter's.

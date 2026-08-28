---
id: PRD-L
status: open
lane: LANE-L
phase: "Phase 0.4 + Phase 1 formatter"
blocked_by: []
labels:
  - epic
  - prd
  - blocking
  - compiler
  - grammar
  - dx
  - fmt
source: docs/roadmaps/flux-roadmap-to-1.0.md §0.4,§3,§12,§13
related_adrs:
  - ADR-0029
---

# PRD-L: Grammar Freeze + `flux fmt`

- **Lane:** LANE-L (Phase 0.4 + Phase 1 formatter, blocking, parallel)
- **Depends on:** none
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §0.4, §3, §12, §13
- **Related ADRs:** ADR-0029 (indentation-based grammar migration), AGENTS.md §3.6
  (grammar transition rules)

## Problem Statement

The lexer already emits `Indent`/`Dedent`/`Newline` layout tokens (ADR-0029), but brace-syntax
sources still live in some fixtures and sample projects alongside the new indentation-based syntax.
Any DX work — syntax highlighting, LSP, formatter — built against the old grammar will need rework.
The roadmap is explicit: the new grammar must be the *only* one CI accepts before the LSP (PRD-O)
and `flux fmt` are built, or that work is wasted. There is also no formatter at all, which for an
indentation-sensitive grammar is a fragmentation risk the moment external contributors touch `.flux`.

## Solution

Finish the ADR-0029 indentation-based grammar migration: delete brace-syntax fixtures, make the new
grammar the only one CI accepts, and ship `flux fmt` (a non-negotiable for an indentation-sensitive
language) before external contributors write `.flux` files.

## User Stories

1. As a Fluff app developer, I want CI to reject brace-syntax `.flux` so that the grammar is
   unambiguous, so that I am never surprised by two valid syntaxes.
2. As a Fluff app developer, I want `flux fmt` to canonicalize my `.flux` files, so that style debates
   never fragment the ecosystem.
3. As a Fluff app developer, I want `flux fmt --check` in CI, so that my committed files stay
   canonical without me running the formatter by hand.
4. As a Flux core engineer, I want brace-syntax fixtures deleted, so that the parser/test surface
   reflects only the shipping grammar.
5. As a Flux tooling author, I want the grammar frozen before I build the LSP (PRD-O), so that I do
   not build against a moving target.
6. As an external contributor, I want one documented, finalized `.flux` surface, so that I can learn
   it once.

## Implementation Decisions

- **Freeze order:** delete brace fixtures → flip CI to reject brace syntax → only then build `flux fmt`.
  Building the formatter against a not-yet-frozen grammar risks rework.
- **Formatter is parser-roundtrip based:** `flux fmt` should parse to the AST/IR and pretty-print from
  the canonical tree, *not* a regex/whitespace heuristic, so output is deterministic and stable as the
  grammar evolves. This reuses the existing lexer/parser.
- **Respect AGENTS.md §3.6:** new surface syntax (keywords, token kinds) only lands via a syntax ADR;
  the lexer keyword map in `crates/flux-parser/src/lexer.rs` remains the choke point. This PRD does not
  add new syntax — it only removes the superseded brace form.
- **CLI surface:** `flux fmt [--check] [<path>]` joins the existing `flux init/dev/build/doc`
  commands (AGENTS.md §3.9); do not invent a separate binary.

## Testing Decisions

- **Good test:** idempotence (fmt(fmt(x)) == fmt(x)), and that formatting a canonical file is a no-op;
  round-trip (parse(pretty-print(parse(x))) preserves the lowered IR). Not tests of formatter internals.
- **Modules to test:** the formatter emit, the CI accept/reject gate for brace syntax, and the
  round-trip property against the `flux-parser`/`flux-ir` lowering.
- **Prior art:** ADR-0029's migration tests and `flux-parser`'s existing layout-token tests are the
  seed. Reuse `flux-parity` fixtures as the round-trip corpus.

## Out of Scope

- New grammar productions for list comprehension / slot composition (those are tracked separately as
  stdlib grammar gaps in PRD-S and PRD-N).
- The LSP itself (PRD-O).
- Diagnostics quality bar = rustc (PRD-S).
- iOS/Android render-tier (PRD-J).

## Further Notes

This PRD is a Phase 0 exit criterion and a hard prerequisite for PRD-O (the VS Code extension /
LSP must target the frozen grammar). The §13 "exactly 0 `unwrap`" gate is owned by PRD-K, not here.

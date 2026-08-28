---
id: PRD-S
status: open
lane: LANE-S
phase: "Phase 1"
blocked_by:
  - PRD-L
  - PRD-K
labels:
  - epic
  - prd
  - compiler
  - language
  - dx
  - diagnostics
  - grammar
source: docs/roadmaps/flux-roadmap-to-1.0.md §3
related_adrs:
  - ADR-0035
  - ADR-0037
  - ADR-0047
---

# PRD-S: Compiler & Language Maturity

- **Lane:** LANE-S (Phase 1 — maps the roadmap §3 "Phase 1" item, unmapped by the LANE table)
- **Depends on:** PRD-L (grammar freeze) for any new grammar productions; PRD-K for error shape
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §3
- **Related ADRs:** ADR-0035 / ADR-0037 (G1–G4 grammar gaps closed), ADR-0047 (monomorphization
  codegen), AGENTS.md §3.11 (diagnostics bar), PRD-K (FluxError)

## Problem Statement

The roadmap's Phase 1 (Compiler & Language Maturity) has no lane in the §12 table, yet it is
foundational to "10x DX": the diagnostic quality bar must be rustc-grade (precise span, one-line what,
why, and a suggested fix) *before* the LSP (PRD-O) and DevTools (PRD-P) assume a diagnostic shape —
retrofitting is more expensive. The type system also lacks ergonomics real apps need: nullable/optional
chaining, structural vs nominal typing for props, and a `Result`/error-propagation story in `.flux`
itself for capability calls that can fail (the runtime already promises "denied grant → typed error,
never a crash" per `capabilities.flux`; the language ergonomics around handling that must be as good).

## Solution

Raise every parse/type error to the rustc quality bar (precise span + what + why + mechanical suggested
fix). Close remaining stdlib grammar gaps as new needs surface, tracking each via ADR → grammar
production → close (list comprehension / iteration syntax for rendering lists; slot/children composition
for containers like `Modal`). Confirm the type-system story for nullable/optional chaining ergonomics,
structural vs nominal prop typing, and a `Result`/error-propagation story in `.flux` for fallible
capability calls. Ship `flux fmt` (also PRD-L — co-owned here as the language-surface item).

## User Stories

1. As a Fluff app developer, I want every parse/type error to show a precise span, a one-line what, a
   why, and a suggested fix, so that I fix errors as fast as Rust developers do.
2. As a Fluff app developer, I want list-comprehension / iteration syntax to render lists, so that I do
   not hand-unroll list builders.
3. As a Fluff app developer, I want slot/children composition for containers like `Modal`, so that I can
   compose subtrees without awkward props.
4. As a Fluff app developer, I want ergonomic nullable/optional chaining, so that I stop fighting the
   type system on optional data.
5. As a Fluff app developer, I want a clear story for structural vs nominal prop typing, so that prop
   shapes compose predictably.
6. As a Fluff app developer, I want a `Result`/error-propagation story in `.flux` for fallible capability
   calls, so that handling a denied grant is as ergonomic as the runtime contract already is.
7. As a Flux tooling author, I want the diagnostic shape fixed before I build the LSP (PRD-O), so that
   the LSP and CLI never disagree on a diagnostic.
8. As a Flux core engineer, I want new grammar productions ADR'd and closed the same way G1–G4 were
   (ADR-0035/0037), so that grammar growth stays disciplined.

## Implementation Decisions

- **Diagnostics first:** the rustc-grade diagnostic shape is locked before PRD-O/PRD-P build on it. This
  is cheaper now than retrofit (roadmap §3 is explicit).
- **Grammar growth is ADR-gated:** any new production (list comprehension, slot/children) goes through
  the same ADR → production → close path as G1–G4 (ADR-0035/0037); the lexer keyword map (AGENTS.md §3.6)
  stays the choke point. PRD-L owns the *freeze*; PRD-S owns *new* productions after freeze.
- **Type ergonomics respect monomorphization:** generics are monomorphized (ADR-0047 handles codegen);
  the nullable/optional and `Result` stories must lower cleanly through the same monomorphization path —
  no bridging types that break codegen.
- **`Result` mirrors the runtime contract:** the in-language error-propagation shape must match PRD-K's
  `FluxError` "denied grant → typed error, never crash" so the language and runtime tell one story.
- **`flux fmt` co-ownership:** the formatter is listed in both PRD-L (freeze) and here (language surface);
  implement once, reference from both.

## Testing Decisions

- **Good test:** diagnostic tests asserting each error class emits span + what + why + suggested fix in a
  snapshot; type-system tests asserting nullable/`Result` lower through monomorphization to the same code
  as hand-written equivalents. Not tests of parser internals.
- **Modules to test:** `flux-parser` / `flux-types` diagnostic emitters, the type-checker ergonomics
  (nullable/optional/`Result`), and the new grammar productions' lowering in `flux-ir`.
- **Prior art:** ADR-0035/0037's closed G1–G4 productions and AGENTS.md §3.11's diagnostic bar are the
  template; `insta` snapshots (AGENTS.md §2.1) are the established diagnostic-test mechanism.

## Out of Scope

- The grammar freeze / deleting brace fixtures (PRD-L).
- The error taxonomy itself (PRD-K) — PRD-S consumes its shape.
- New stdlib primitives (PRD-N) — only the language features they may need.
- DevTools / LSP (PRD-P / PRD-O).

## Further Notes

PRD-S fills the gap the §12 LANE table leaves: "Phase 1 — Compiler & Language Maturity" has no lane.
It is the prerequisite that makes PRD-O's LSP trustworthy. Diagnostic quality is the cheapest to build
now and the most expensive to retrofit.

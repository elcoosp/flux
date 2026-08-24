# ADR parser-grammar-extensions: productions FLUX-003 added beyond Appendix B

**Status:** Accepted (implemented in `flux-parser`)
**Date:** 2026-08-24
**Author:** parser agent (FLUX-003)
**Scope:** `crates/flux-parser/` (owned per `agents-boundaries-contract.md`)
**Addresses:** ASR-2 (developer velocity depends on precise parse diagnostics)
**Relates to:** `stdlib-grammar-gaps.md` (G1–G4), `appendix-b-grammar-repairs.md`

## Context and Problem Statement

FLUX-003 implements the parser against Appendix B (`docs/spec/mlp-appendices.md`
§B.1–B.2). Appendix B's own §B.3 examples, and the twelve stdlib files authored
under FLUX-010, use constructs for which §B.1–B.2 has no production. AGENTS.md
§6 forbids improvising architecture and requires an ADR when the spec is
silent. This ADR records every production `crates/flux-parser/src/flux.pest`
carries beyond the literal text of Appendix B, so the appendix can be amended
(or the parser corrected) in one pass.

Each extension is marked in `flux.pest` with its `[Pn]` tag.

## Decision

Accept the productions below. Every one is required by normative material
elsewhere in the spec — an Appendix B.3 example, an Appendix F prop contract,
or mlp-spec §18/§24 — so accepting them makes the grammar consistent with the
rest of the specification rather than extending the language.

### Closing the FLUX-010 gaps (G1, G2, G4)

| Gap | Production | Required by |
|---|---|---|
| G1 | `const_binding = const_path ~ "=" ~ expr` | mlp-spec §18.6 `Color.red = RGB(…)` |
| G2 | `prop_decl` / `param` gain an optional `"=" ~ expr` default | Appendix F.1–F.7 (`gap: Float = 0`) |
| G4 | `fn_name = ident \| operator` | Appendix B.3.2 (`fn +(a: T, b: T) -> T`) |

G3 (record/tuple variant construction in value position) needs no new
production: positional construction is `postfix_expr` applied to an `ident`,
which the existing call grammar already covers.

### Productions the Appendix B.3 examples require

- **[P9] `block_expr`** — a bare block in value position is a zero-argument
  closure. B.3.1 writes `onClick: { count = count + 1 }`. Lowered to a
  `Lambda` with no parameters.
- **[P10] `first_variant`** — the leading `|` of a `type` declaration is
  optional on the first variant only, so the stdlib's single-variant newtypes
  (`type Ref[T] = Ref(T)`, prelude.flux) parse while `type T = A B` still does
  not.
- **`props_block`, `prop_entry`** — B.3.7 writes both a parenthesised prop
  list (`component Avatar(url: String, size: Float)`) and a trailing prop
  block (`Image(url) { width: size, … }`).
- **`ellipsis`** — B.3.8 writes `onClick: { ... }` as an elided body.

### Extensions the stdlib requires

- **[P11] `module_state`** — `stdlib/platform.flux` declares
  `state platform: String = "ios"` at module level: a runtime-bound module
  value. Appendix B's `statement` has no such production. Parsed as
  `Decl::State`. Recorded here as **gap G5**; the orchestrator should decide
  whether Appendix B gains a `module_state` production or `platform.flux`
  is rewritten as a `fn`.

### Robustness limit

- **G6 — `MAX_NESTING_DEPTH = 16`.** pest descends the whole
  expression-precedence chain at every block nesting level (~100 KB of stack
  per level), so unbounded nesting aborts the process rather than returning an
  error. The parser therefore rejects input nested deeper than 16 blocks with
  an actionable diagnostic, checked lexically before parsing. The value is the
  depth measured to parse safely on the ~2 MB stacks test harnesses use; real
  view trees nest far less. If deeper nesting is ever needed, the fix is to
  parse on an explicitly-sized stack, not to raise the constant blindly.

## Considered Options

1. **Accept the productions and record them here (chosen).** The parser
   accepts every §B.3 example and all twelve stdlib files, so FLUX-015 can
   validate the stdlib as authored.
2. **Implement Appendix B literally.** Would reject all ten §B.3 examples and
   six stdlib files — the appendix's own examples would not parse.
3. **Amend Appendix B first.** Correct, but `mlp-appendices.md` is
   orchestrator-owned and concurrently edited; a parser-agent edit would
   collide. This ADR is the hand-off instead.

## Consequences

**Positive:** All ten Appendix B.3 examples and all twelve stdlib files parse.
Diagnostics carry what/where/why/how per AGENTS.md §3.7. Deep input fails with
an error instead of aborting the process.

**Negative:** Until Appendix B is amended, `flux.pest` is the de-facto grammar
of record for these constructs. Every extension is tagged `[Pn]` in the grammar
and tested (`tests/appendix_b_examples.rs`, `tests/stdlib.rs`) to keep the
divergence visible and enumerable.

**Follow-up for the orchestrator:** ratify G1/G2/G4, decide G5 (module-level
`state`), and fold P9–P11 into Appendix B §B.2.

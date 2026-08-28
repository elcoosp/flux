# ADR-0046-dream-syntax-handwritten-parser: replacing pest with a hand-written lexer + recursive-descent parser

**Status:** Accepted (implemented in `flux-parser`; dream syntax is now the only surface syntax)
**Date:** 2026-08-27
**Author:** flux-parser agent (FLUX-003)
**Scope:** `crates/flux-parser/` (lexer + parser), all `.flux` sources, and every inline test fixture across the workspace
**Addresses:** ASR-1 (write-once UI language with a fast, precise parser), ASR-2 (precise parse diagnostics)
**Relates to:** `ADR-0035-parser-grammar-extensions.md`, `ADR-0029-appendix-b-grammar-repairs.md`, `docs/counter-syntax-dream.md`

## Context and Problem Statement

Flux's parser was generated from `flux.pest` (a PEG grammar). The project's
intended surface syntax is the indentation-delimited "dream" syntax described in
`docs/counter-syntax-dream.md` (`compo`, the `$` state sigil, spaced-prop view
calls, `||` lambdas, and brace-free component bodies). The pest grammar encoded
an older brace-delimited syntax (`component X { … }`) that diverged from the
dream spec, forcing a hand-written superset and a growing pile of `[Pn]` grammar
extensions (see ADR-0035).

Two problems forced a rewrite:

1. **Spec drift.** The shipped grammar accepted a different surface than the
   dream syntax the rest of the system (codegen, type checker, runtime, parity
   harness) targets. Keeping pest in sync with `docs/counter-syntax-dream.md`
   meant maintaining two sources of truth.
2. **Performance / control.** The AGENTS.md performance budgets (parse 500 lines
   < 5 ms) and the need for byte-accurate `Span` diagnostics are far easier to
   guarantee with a hand-written lexer + recursive-descent parser that allocates
   nothing on the hot path and returns spans directly, than with a generated
   PEG parser that hides the token stream and complicates span recovery.

The decision: **replace `flux.pest` entirely** with a hand-written lexer
(`lexer.rs`) and a recursive-descent parser (`parser.rs`) that implement the
dream syntax as the single normative surface. Brace-delimited old syntax is no
longer accepted.

## Decision

1. **`flux.pest` is deleted.** The `pest`/`pest_derive` dependencies are removed
   from `flux-parser/Cargo.toml`. The grammar now lives only in
   `docs/counter-syntax-dream.md` (the spec) and its realization in
   `lexer.rs`/`parser.rs` (the implementation). Any future syntax change must
   update the dream doc and the hand-written parser together; `tests/` assert
   every §B.3 example against the parser.

2. **Indentation delimits structure.** The lexer emits `Indent` / `Dedent`
   markers from column changes between logical lines (blank lines and comment
   lines are skipped). `compo` bodies, `Column { … }` blocks, `fn` bodies, and
   `match` arms are all indentation-delimited. Braces remain only for *inline*
   blocks: `onMount { … }`, `onClick: { … }`, `fn(msg) { … }`, `match { … }`,
   `when { … } otherwise { … }`, `if { … } else { … }`, and destructuring
   patterns such as `let (users, { refetch }) = …`.

3. **Lexer is a hand-written state machine.** `lexer.rs` produces a flat
   `Vec<Token>` (no boxing, no allocation beyond the token buffer) with each
   token carrying a `Span` over the original source bytes. Operators, string
   interpolation (`"…{expr}…"`), the `$` state sigil, and comments are all
   lexed explicitly. Escapes and nested interpolations are handled in
   `parser.rs::string_lit` / `interp_expr`, which re-lexes the interpolation
   slice and shifts spans back into whole-source coordinates.

4. **Parser is recursive descent.** `parser.rs` exposes a single public entry
   point, `parse(source, file_id, path) -> Result<Program, ParseError>`, used
   by every crate (`flux-types`, `flux-ir`, `flux-cli`, `flux-codegen-*`,
   `flux-parity`, `flux-devserver`). No production code uses `unwrap`/`expect`;
   parse failures return a `ParseError` carrying a `Span` plus what/where/why/how.

5. **Node-ID stability is preserved.** Component/state/prop/expr node ids are
   still derived from `(parent_id, kind, span, key)` via
   `flux_ir::compute_node_id()` (or the `blake3` formula directly), so hot-swap
   and state preservation are unaffected by the parser swap.

6. **`MAX_NESTING_DEPTH = 16` stays parser-internal** (carried over from
   ADR-0035). The brace-depth check rejects input nested deeper than 16 blocks
   with an actionable diagnostic before descending.

## Considered Options

1. **Keep pest, extend the grammar to dream syntax.** Rejected: a PEG grammar
   cannot naturally express indentation sensitivity without awkward
   whitespace rules, and it obscures the token stream we need for byte-accurate
   spans. Maintaining pest + the dream doc as two sources of truth was the
   original drift problem.

2. **Keep pest for old syntax and add a separate dream parser.** Rejected:
   dual surface syntax means dual test matrices, dual codegen inputs, and
   permanent ambiguity about which syntax is normative. The spec is unambiguous
   that dream is the surface.

3. **Hand-written lexer + recursive-descent for dream only (CHOSEN).** One
   surface, one parser, spans are direct, allocation is under control, and the
   grammar is the implementation. This is the fastest path to a parser that
   meets the AGENTS.md budgets and diagnostic standards.

## Consequences

### Positive
- Dream syntax is now the single normative surface; the spec and parser cannot
  drift.
- Spans are byte-accurate and allocation-free on the hot path; parse diagnostics
  meet AGENTS.md §3.7 (what/where/why/how).
- All 402 workspace tests pass under dream syntax, including the 10 Appendix B.3
  parity examples, the flux-parity snapshot suite, and the flux-types type
  checker.

### Negative / costs
- The parser is now hand-maintained; grammar changes require editing
  `parser.rs`/`lexer.rs` rather than a `.pest` file. Mitigated by the
  `tests/` suite that pins every §B.3 example and the `dream_syntax.rs` /
  `appendix_b_examples.rs` acceptance tests.
- Every `.flux` source and inline test fixture across the workspace was migrated
  from old to dream syntax (stdlib, examples, flux-cli, flux-parity,
  flux-devserver, flux-ir, flux-types, flux-codegen-*). This was a large,
  mechanical but careful migration; see the CHANGELOG entry.

### Neutral
- `blake3`, `thiserror`, `tracing` remain dependencies; `pest`/`pest_derive`
  are gone.

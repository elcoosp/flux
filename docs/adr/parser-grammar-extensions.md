# ADR parser-grammar-extensions: keeping flux.pest and Appendix B in sync

**Status:** Accepted (implemented in `flux-parser`; reconciled with Appendix B)
**Date:** 2026-08-24
**Author:** parser agent (FLUX-003)
**Scope:** `crates/flux-parser/` and `docs/spec/mlp-appendices.md` Appendix B
**Addresses:** ASR-2 (developer velocity depends on precise parse diagnostics)
**Relates to:** `stdlib-grammar-gaps.md` (G1–G4)

## Context and Problem Statement

FLUX-003 implements the parser against Appendix B (`docs/spec/mlp-appendices.md`
§B.1–B.2). The §B.3 examples and the twelve stdlib files use constructs that
the *original* Appendix B did not spell out. At first the parser added them as
extensions tagged `[Pn]` in `crates/flux-parser/src/flux.pest`, with this ADR
recording the divergence. The resolution taken on 2026-08-24 was to **rewrite
Appendix B.1/B.2 so it matches the tested grammar 1:1**, making the spec the
source of truth and removing the divergence entirely.

One of those extensions — a module-level `state` form (`module_state`,
library-side `Decl::State`), added so `stdlib/platform.flux` could declare
`state platform: String = "ios"` — was later **reverted** (see the
"Module state" decision below) in favour of an ordinary `fn`.

This ADR records the reconciliation and the one concern that remains
parser-internal.

## Decision

1. **Appendix B is normative and complete.** `docs/spec/mlp-appendices.md` §B.1
   and §B.2 were rewritten to contain every production the parser needs,
   transcribed from `flux.pest`. The two are kept in sync; any future grammar
   change must update both, and `tests/appendix_b_examples.rs` asserts every
   §B.3 example against the parser.

2. **There is no module-level `state` form.** File-scope declarations are
   `import`, `use`, `component`, `fn`, `type`, `trait`, `capability` and
   associated `const`. A runtime-bound module value such as the platform tag is
   written as a `fn` (`fn platform() -> String { … }` in stdlib/platform.flux)
   and queried by calling it (`if platform() == "ios" { … }`, Appendix B.3.8).
   This keeps the language without a file-scope state form and keeps the parser
   `Decl` enum free of a `State` variant. The earlier `module_state` form
   (gap G5) was accepted into the grammar and then reverted after review; the
   `Decl::State` variant and `module_state` rule were removed from the parser
   and Appendix B.

3. **`MAX_NESTING_DEPTH = 16` stays parser-internal (not a grammar rule).**
   pest descends the whole expression-precedence chain at every block level
   (~100 KB of stack per level), so unbounded nesting aborts the process
   instead of returning an error. The parser therefore rejects input nested
   deeper than 16 blocks with an actionable diagnostic, checked lexically
   before parsing. The value is the depth measured to parse safely on the
   ~2 MB stacks test harnesses use; real view trees nest far less. If deeper
   nesting is ever needed, the fix is to parse on an explicitly-sized stack,
   not to raise the constant blindly. This is recorded as **G6**.

## Considered Options (module state)

1. **Module-level `state` form (rejected after trial).** Added as
   `module_state`/`Decl::State`, tested, and documented in §B.2. Rejected on
   review: a file-scope `state` introduces a new declaration kind with no
   counterpart in the spec's other layers and is unnecessary — the same
   information is a read-only `fn`.
2. **`fn` returning the value (chosen).** `fn platform() -> String { … }`
   reads naturally at call sites (`platform()`), needs no grammar or AST
   addition, and keeps the language surface smaller.

## Considered Options (grammar reconciliation)

1. **Reconcile Appendix B with the tested grammar (chosen).** One source of
   truth; `flux.pest` and §B.2 cannot drift.
2. **Keep the divergence, track it in this ADR.** Rejected: two parallel
   grammars invite silent drift and were the reason the ADR was needed at all.

## Consequences

**Positive:** Appendix B and `flux.pest` are now identical in content. All ten
§B.3 examples and all twelve stdlib files parse. Diagnostics carry what/where/
why/how per AGENTS.md §3.7. Deep input fails with an error instead of
aborting. The language has no file-scope state form.

**Negative:** none outstanding.

**Follow-up:** any future grammar edit must touch both `flux.pest` and
Appendix B, then re-run `cargo nextest run -p flux-parser`.

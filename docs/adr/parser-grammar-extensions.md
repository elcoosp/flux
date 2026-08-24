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
recording the divergence. The cleaner resolution — taken on 2026-08-24 — was to
**rewrite Appendix B.1/B.2 so it matches the tested grammar 1:1**, making the
spec the source of truth and removing the divergence entirely.

This ADR now records that reconciliation and the one concern that remains
parser-internal.

## Decision

1. **Appendix B is normative and complete.** `docs/spec/mlp-appendices.md` §B.1
   and §B.2 were rewritten to contain every production the parser needs,
   transcribed from `flux.pest`. The two are kept in sync; any future grammar
   change must update both, and `tests/appendix_b_examples.rs` asserts every
   §B.3 example against the parser.

2. **What the grammar added beyond the *original* B.2** (now folded in, recorded
   for traceability):

   | Tag | Production | Required by |
   |---|---|---|
   | G1 | `const_binding = const_path ~ assign_op ~ expr` | mlp-spec §18.6 `Color.red = RGB(…)` |
   | G2 | `prop_decl`/`param` optional `assign_op ~ expr` default | Appendix F.1–F.7 (`gap: Float = 0`) |
   | G4 | `fn_name = ident \| operator` | Appendix B.3.2 (`fn +(a, b)`) |
   | P9 | `block_expr` — bare block as a zero-arg closure | B.3.1 `onClick: { … }` |
   | P10 | `first_variant` — optional leading `\|` on first ADT variant | stdlib newtypes `type Ref[T] = Ref(T)` |
   | P11 | `module_state` — module-level `state` | `stdlib/platform.flux` `state platform: String = "ios"` |
   | — | `ellipsis`, `props_block` + `prop_entry`, `lambda`, postfix `field_access` | B.3.6–B.3.8 |

   G3 (positional ADT/tuple construction) needs no production: it is
   `postfix_expr` applied to an `ident` (the call grammar).

3. **`module_state` (formerly gap G5) is accepted into the spec.** It reuses
   `state_decl` and lowers to `Decl::State`. It is the only production not in
   the *original* B.2; the orchestrator ratified it by accepting this ADR's
   reconciliation.

4. **`MAX_NESTING_DEPTH = 16` stays parser-internal (not a grammar rule).**
   pest descends the whole expression-precedence chain at every block level
   (~100 KB of stack per level), so unbounded nesting aborts the process
   instead of returning an error. The parser therefore rejects input nested
   deeper than 16 blocks with an actionable diagnostic, checked lexically
   before parsing. The value is the depth measured to parse safely on the
   ~2 MB stacks test harnesses use; real view trees nest far less. If deeper
   nesting is ever needed, the fix is to parse on an explicitly-sized stack,
   not to raise the constant blindly. This is recorded as **G6**.

## Considered Options

1. **Reconcile Appendix B with the tested grammar (chosen).** One source of
   truth; `flux.pest` and §B.2 cannot drift.
2. **Keep the divergence, track it in this ADR.** Rejected: two parallel
   grammars invite silent drift and were the reason the ADR was needed at all.
3. **Parser diverges from the spec.** Rejected by AGENTS.md §6 (no
   improvisation when the spec is silent — and here the spec is no longer
   silent).

## Consequences

**Positive:** Appendix B and `flux.pest` are now identical in content. All ten
§B.3 examples and all twelve stdlib files parse. Diagnostics carry what/where/
why/how per AGENTS.md §3.7. Deep input fails with an error instead of
aborting.

**Negative:** `module_state` widens the language beyond the original B.2; it is
deliberate (required by the stdlib) and now documented in §B.2 Notes.

**Follow-up:** any future grammar edit must touch both `flux.pest` and
Appendix B, then re-run `cargo nextest run -p flux-parser`.

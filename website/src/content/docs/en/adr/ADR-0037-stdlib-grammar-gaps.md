# ADR-0037-stdlib-grammar-gaps: Appendix B gaps surfaced by the FLUX-010 stdlib

**Status:** Draft (flagged to orchestrator; do not edit)
**Date:** 2026-08-24
**Author:** stdlib agent (FLUX-010)
**Scope:** `stdlib-` (create-only per `agents-boundaries-contract.md` R9)

## Context and Problem Statement

FLUX-010 requires authoring the 12 stdlib `.flux` files per mlp-spec §18.3
and the Appendices. The parser crate (`flux-parser`) is still a stub at the
time of writing (FLUX-003 is a later Phase 1 issue), so parse validation is
deferred to FLUX-015. While authoring, the stdlib necessarily uses several
constructs that the spec *shows* (in §18.6, B.3.2, F.1–F.7, §24.1) but that
Appendix B's grammar (`docs/spec/mlp-appendices.md` §B.1–B.2) does **not**
yet provide a production for. These are gaps between the prose examples and
the formal grammar, not authoring mistakes. This ADR records them so the
parser (FLUX-003) and the stdlib validator (FLUX-015) have an explicit list.

The stdlib files are written to match the *prose/examples* (the behavioral
source of truth the examples represent) and annotated with the relevant gap
id (G1–G4) so FLUX-015 can verify each construct once the parser supports it.

## Gaps

### G1 — Top-level associated constant binding `Name.field = expr`
§18.6 shows:
```
Color.red = RGB(1.0, 0.0, 0.0)
Font.body = Font("", 17.0, Regular, Normal)
```
Appendix B has `statement = import_decl | use_decl | component_decl |
fn_decl | type_decl | trait_decl | capability_decl` and no production for a
top-level `path "=" expr` binding. Decision needed: is this a module-level
associated binding (constant in the type's namespace) or sugar for a `fn`?
stdlib uses G1 in `color.flux` and `font.flux`.

### G2 — Prop default values in `prop_decl`
Appendix F.1–F.7 specify optional props with defaults, e.g.
`font: Option[Font] = None`, `gap: Float = 0`, `enabled: Bool = true`,
`text: String = ""`. Appendix B's `prop_decl = ident ":" type` has no
`"=" expr` clause, and `params` likewise. stdlib encodes every Appendix F
default verbatim (text.flux, button.flux, column.flux, row.flux,
text_field.flux). Decision needed: extend `prop_decl`/`param` with an
optional `"=" ~ expr` default.

### G3 — Record/tuple variant construction in value position
§18.6 shows `Font { family: "", size: 17.0, weight: Regular, style: Normal }`
(record literal) as a value. Appendix B only provides `list_lit`, literals,
and `call_expr` for values; a bare record literal is not a production.
stdlib avoids G3 by using **positional** variant construction
`Font("", 17.0, Regular, Normal)` (a `call_expr` of the variant name), which
is already exercised by the §18.6 `RGB(1.0, 0.0, 0.0)` examples. The G3
record-literal form remains unresolved for users who prefer it.

### G4 — Symbolic operator method names in `trait`/`fn` decls
B.3.2 and §18.2 show trait methods named `+`, `-`, `==`, `!=`:
`trait Numeric[T] { fn +(a: T, b: T) -> T ... }`. Appendix B's `method_decl`
and `fn_decl` use `ident` for the name; `+`/`-`/`==`/`!=` are not `ident`
and are not listed as operators in the grammar. Decision needed: add
operator symbols to the method/function name grammar. stdlib uses G4 in
`traits.flux`.

## Considered Options (per gap, for the orchestrator)

For each gap the trade-off is the same: (A) extend Appendix B to cover the
construct the examples already use, vs. (B) change the stdlib to only use
constructs Appendix B already covers. The stdlib took the path of matching
the examples (which are the intended user-facing surface) and flagging the
grammar gap here, because FLUX-010's acceptance bar is "Syntax conforms to
Appendix B **by manual review** (parse-validation happens in FLUX-015)" and
"if a construct is genuinely unparseable per Appendix B, write an ADR and
flag to the orchestrator." That is exactly what this ADR does.

## Decision Outcome

**Chosen (pending orchestrator ratification):** Treat G1–G4 as grammar
specification gaps to be closed by FLUX-003 (extend Appendix B + parser),
not as stdlib defects. The stdlib files are annotated with the gap ids so
FLUX-015 can confirm each construct parses once the parser supports it. The
orchestrator may instead direct the stdlib to be rewritten to avoid a gap
(e.g. drop G1 constants, drop G4 operator methods) — but that would diverge
from the §18.6 / B.3.2 examples the spec presents as canonical.

## Consequences

**Positive:** The parser and stdlib validator have an explicit, reviewed
list of grammar constructs to support; no silent divergence between the
stdlib examples and the grammar.
**Negative:** Until FLUX-003 closes G1–G4, the stdlib files will not fully
parse; FLUX-015 is expected to track this ADR.

## References
- mlp-spec §18.3 (module system, stdlib boundary), §18.6 (Color/Font),
  §18.2 (traits), §24.1 (capabilities).
- mlp-appendices Appendix B (grammar), Appendix F (adapter prop contracts).
- `agents-boundaries-contract.md` FLUX-010 (author) and FLUX-015 (validate).

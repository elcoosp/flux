# ADR-0052: Structural (width-subtyping) record typing

- **Status:** Accepted
- **Supersedes:** —
- **Superseded by:** —
- **Related issues:** FLUX-054
- **Related ADRs:** ADR-0029 (grammar freeze), ADR-0051 (optional chaining)

## Context

Flux prop values and component arguments are records. Historically the type
checker unified records **nominally**: `unify_records` required `fx.len() ==
fy.len()` — the two record shapes had to carry exactly the same fields, in the
same order. That rejected the common and useful case where a value carries
*extra* fields beyond what the receiving position declared:

```flux
compo C
  state p: { x: Int } = { x: 1, y: 2 }
  Text("ok")
```

Here `{ x: 1, y: 2 }` is a perfectly good `{ x: Int }` — the extra `y` field is
harmless. Rejecting it forces callers to strip fields by hand and breaks
composition (a richer record cannot be passed where a narrower one is expected).

FLUX-054 asks for **structural vs nominal prop typing**. The decision recorded
here: Flux records are **structural** — a value is assignable to a record type
when it provides at least the fields the type requires, with compatible types.

## Decision

1. **Width-subtyping for records.** `unify_records` (crates/flux-types/src/
   unify.rs) no longer requires equal field counts. Every field declared by the
   *expected* (`expected ⊆ found`) type must be present in the *found* type with
   a compatible type; fields present only in `found` are permitted. The reverse
   — a `found` record missing a required `expected` field — remains a type
   error.

2. **Anonymous record literals.** To make structural records usable, the
   grammar gains anonymous record literals `{ x: 1, y: 2 }` (FLUX-054 / this
   ADR). The parser (crates/flux-parser/src/parser.rs) emits
   `ExprKind::Record { name: "", fields }` when a `{ ... }` primary's entries
   are all `ident: expr` and there is no `name =>` block-param header. The
   empty `name` signals an anonymous (structural) record; the checker's
   `ExprKind::Record` arm already produces a `TcType::Record`, so no checker
   change beyond `unify_records` is required. Named ADT record constructors
   (`RGB(1, 0, 0)`) continue to unify by their type name as before.

3. **Scope boundary.** This ADR covers record *values* and *prop/argument
   typing* only. Component *declaration* prop rows remain ordered and named.
   No new VM opcode or IR node is introduced — anonymous records lower through
   the existing `ExprKind::Record` path.

## Consequences

- Positive: richer records compose into narrower positions; callers stop
  hand-stripping fields; prop passing is positional/extensible.
- Negative: a genuinely mistyped field name in a wider record is no longer
  caught at the narrower position (only at the field's own use site). This is
  the standard structural-typing trade-off and is accepted.
- The `missing field` error is preserved, so under-populated records are still
  rejected with a precise diagnostic.

## Alternatives considered

- *Nominal records with explicit `extend`.* Rejected: more syntax, less
  ergonomic, defeats the composition goal.
- *Keep nominal, require exact width.* Rejected: this is the status quo FLUX-054
  explicitly rejects.
- *Row polymorphism with explicit `|` tail.* Rejected for MLP scope: adds a
  type-level surface with no runtime payoff yet; structural subset covers the
  reported need.

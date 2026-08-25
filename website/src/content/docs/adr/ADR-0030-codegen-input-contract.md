---
title: "ADR-0030-codegen-input-contract: codegen takes `(LoweredIr, Ast)`, not `LoweredIr` alone"
---

# ADR-0030-codegen-input-contract: codegen takes `(LoweredIr, Ast)`, not `LoweredIr` alone

- **Status:** Accepted (created 2026-08-25 by the orchestrator)
- **Supersedes the recommendation in:** `docs/adr/ADR-0031-codegen-kotlin-loweredir-name-gap.md`
  (that ADR proposed a `flux-ir` change — Option C — to expose component names;
  this ADR rejects that option and settles the contract another way)
- **Related:** FLUX-020 (codegen-swift, DONE), FLUX-021 (codegen-kotlin, PENDING),
  ADR-0034 (node-ID bridge), `docs/agents-boundaries-contract.md` Batch 5
  (which specified `codegen(&LoweredIr) -> String`).

## Context

The boundary contract (Part 2, Batch 5) specifies the release codegen entry point
as:

```rust
pub fn codegen(lowered: &LoweredIr) -> String
```

`LoweredIr` as produced by lowering carries only numeric `ComponentId`s and **drops
runtime values** (string literal text, generics, `@pure`, prop/state types) to keep
the arena compact (Appendix C §C.1). The Kotlin codegen agent therefore raised
`docs/adr/ADR-0031-codegen-kotlin-loweredir-name-gap.md`, which concluded FLUX-020/021 were
*BLOCKED* until a `flux-ir` change (Option C) exposed component names.

Between that ADR and dispatch, **FLUX-020 (codegen-swift) was implemented and
committed** and resolved the problem without touching `flux-ir`: it changed the
signature to

```rust
pub fn codegen(lowered: &LoweredIr, ast: &flux_parser::Ast) -> String
```

and recovers names / generics / string interpolations / `@pure` from the originating
`Ast` through a `bridge` module built on the ADR-0027 node-ID bridge. The arena
provides tree *structure*; the AST provides *semantics*. The ADR-0027 node-ID bridge
guarantees the two are joined by identical `NodeId`s, so any lowered node maps back
to its surface construct.

This makes the "component-name gap" described in the kotlin ADR a non-issue for
codegen: codegen never needed the name from `LoweredIr` — it reads it from the AST.

## Decision Drivers

- **Boundary contract (R3):** a codegen crate must not make public-API changes to a
  sibling crate (`flux-ir`) that other agents consume. The ADR-0027 node-ID bridge is
  the *intended* cross-crate join, so codegen should consume it rather than fork
  `flux-ir`.
- **Parity (FLUX-023):** dev VM execution and release codegen must produce identical
  state. Both Swift and Kotlin backends must consume the *same* input contract, or the
  parity harness cannot compare them. Diverging the Kotlin backend onto a `flux-ir`
  name map would split the contract and break parity.
- **Gap G3 independence:** closing Gap G3 (`arena.string_table()` populated, see
  `docs/adr/ADR-0033-flux018-string-table-gap.md`) is needed by the **dev server / wire codec
  / adapters** (text rendering on the wire). It is *not* required by codegen, which
  already gets text from the AST. So codegen (FLUX-021) must not be gated behind G3.
- **Precedent (FLUX-020):** a working, green, 1106-line Swift implementation already
  validates this design end-to-end with snapshot tests. Re-deriving names inside
  codegen (the rejected Option A in the kotlin ADR) would duplicate that and is
  strictly worse.

## Decision Outcome

**Adopt `codegen(&LoweredIr, &Ast) -> String` as the canonical release-codegen
contract, mirrored for both backends.** Specifically:

1. **FLUX-021 (codegen-kotlin) mirrors FLUX-020 exactly:** same signature
   `codegen(&LoweredIr, &Ast)`, a `bridge` module reconstructing the node-ID → AST
   mapping via `flux_syntax::compute_node_id` with the ADR-0027 tags
   (`EXPR_TAG = 10`, component-decl tag), and emits Compose source from the recovered
   names/types. No `flux-ir` change. No reconstruction of an internal name map.
2. **The kotlin ADR's blocking conclusion is rescinded.** FLUX-021 is **not** blocked
   by `flux-ir`; it is unblocked by the FLUX-020 precedent. Its ADR's "Action
   required" / "Option C" rows are superseded by this ADR.
3. **Gap G3 (component/primitive names in `LoweredIr`, and the arena string table)
   remains a real fix but is scoped to the dev server + wire + adapters**, not to
   codegen. It is tracked separately (`ADR-0033-flux018-string-table-gap.md`, now closed in
   `flux-ir`) and is a prerequisite for FLUX-019, not for FLUX-021.
4. **Update Batch 5 of the boundary contract** (the `(lowered) -> String` shape) to
   the settled `(lowered, ast) -> String` form so future agents read the right
   signature. (Contract text edit is orchestrator-owned per §1.2.)

The FLUX-021 agent MUST:
- take `codegen(&lowered: &LoweredIr, ast: &Ast) -> String`;
- import and follow `flux-codegen-swift/src/{bridge,codegen}.rs` as the reference;
- recover component/primitive names, generics, prop/state types, `@pure` and string
  interpolations from `ast` via the ADR-0027 bridge, never from `flux-ir`.

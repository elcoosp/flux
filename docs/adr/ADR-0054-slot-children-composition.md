# ADR-0054: Slot/children composition for containers

- **Status:** Accepted
- **Date:** 2026-08-29
- **Supersedes / related:** PRD-S (deferred slot/children composition),
  FLUX-052, ADR-0029 (frozen indentation grammar), FLUX-038 (`Modal`/`Sheet`/
  `Dialog` containers)

## Context

Containers like `Modal`/`Sheet`/`Dialog`/`Column`/`Row` need a children/slot
composition model so a component can take child content. PRD-S deferred it as
ADR-gated. FLUX-052 asked for an ADR + grammar + type + lower support + parity
tests.

On inspection the language surface for **typed slot/children composition
already exists** in the frozen indentation grammar: a view expression accepts a
trailing indented block, which the parser lowers to `ExprKind::Call { trailing:
Some(Block) }`, and the IR lowers that trailing block's UI-producing
expressions into the node's `children` (see `lower_block` in
`crates/flux-ir/src/lower/mod.rs`). So `Modal { Column { Text("hi") } }`
already composes a typed child tree with no new grammar, AST, or IR node. The
only thing missing was the paper trail and a parity fixture pinning the
dev/release shape.

This ADR records that decision: **slot/children is the trailing-block model;
no new syntax is introduced.** A container is just a component/adapter call
that happens to place its trailing block's children under itself in the shadow
tree. `FLUX-038`'s `Modal`/`Sheet`/`Dialog` already exploit this — they are
registered in the checker prelude and codegen emits their native surface with
the trailing block as the child body.

## Decision

1. **Children are the trailing indented block.** A call `C { … }` passes its
   block as `Call.trailing`; the lower pass emits each UI-producing child as a
   `Child` of `C`'s node. This is the "slot" — there is exactly one
   content slot per container, which matches the MLP container vocabulary
   (`Modal`/`Sheet`/`Dialog`/`Column`/`Row`/`Stack` take a single body).
2. **No new grammar / AST / IR node.** The existing `Call.trailing` +
   `lower_block` path is the entire mechanism. FLUX-052 therefore closes with
   docs + a parity fixture, not a parser/IR change.
3. **Typed children.** Because the trailing block is ordinary surface syntax,
   its children type-check against the component body rules already in force
   (state/prop/UI expressions). No separate "slot type" is introduced.
4. **Parity.** `crates/flux-parity` gains a fixture (`B.._CONTAINER_CHILDREN`)
   that renders a `Modal` wrapping a `Column` of two `Text`s and asserts the
   dev / Swift / Kotlin paths agree on the nested child tree.

## Consequences

- Positive: container composition works today and is pinned by parity; the
  deferred feature needed no new compiler surface.
- Negative: only a single content slot per container exists. Multi-slot
  containers (e.g. a `List` with `header`/`footer` slots) would need a named
  slot extension — explicitly out of scope for MLP and for this ADR.
- The frozen grammar stays frozen: this ADR merely documents existing behavior.

## Verification

- `crates/flux-parity` fixture `B.._CONTAINER_CHILDREN` lowers + asserts
  structural dev/Swift/Kotlin parity for a container-with-children tree.
- No new unit test in `flux-parser`/`flux-ir` is required because the
  production path is unchanged; the parity fixture is the regression guard.

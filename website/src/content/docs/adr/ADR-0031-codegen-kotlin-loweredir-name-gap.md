---
title: "ADR-0031-codegen-kotlin-loweredir-name-gap: `LoweredIr` does not carry the component names codegen needs"
---

# ADR-0031-codegen-kotlin-loweredir-name-gap: `LoweredIr` does not carry the component names codegen needs

- **Status:** Proposed (created 2026-08-25 by the FLUX-021 codegen-kotlin agent)
- **Blocks:** FLUX-021 (Kotlin/Compose codegen), and by symmetry FLUX-020 (Swift/SwiftUI codegen) — both are specified with the same entry point.
- **Related:** `docs/adr/ADR-0033-flux018-string-table-gap.md` (Gap G3), FLUX-018 (lowering),
  `docs/agents-boundaries-contract.md` §15 (Gap G3) and line 699 (`AdapterRegistry` maps
  `ComponentId` → adapter *"using the string table from the `Init` frame"*),
  Appendix C §C.1 (IR schema), Appendix F (adapter contracts), ADR-0003, ADR-0004.

## Context and Problem Statement

FLUX-021 specifies the release-mode Kotlin backend as:

```rust
pub fn codegen(lowered: &LoweredIr) -> String
```

The mapping rules it must implement are name-driven: `Column` → `Column(spacing = N.dp)`,
`Row` → `Row(spacing = N.dp)`, `Text` → `Text(text, …)`, `Button` →
`Button(onClick = …) { Text(…) }`, `TextField` → `TextField(value =, onValueChange =)`,
`Router` → `NavHost`, and a user component `Counter` → `@Composable fun Counter(…)`.
Every one of those decisions requires knowing **which** component or primitive a node is.

`LoweredIr` as currently produced does not carry that information.

### Gap A — component/primitive names are discarded

`IRArena` stores `component_id: ComponentId`, and `ComponentId` is a bare integer
(`pub type ComponentId = u32;` — `crates/flux-syntax/src/ids.rs`). Lowering builds the
name → id mapping in `Lowerer::name_to_component`, but that map is **dropped in
`Lowerer::finish()`**: it is never interned into the arena's `StringTable`, and it is not
exposed on `LoweredIr` (which carries only `arena`, `closures`, `instances`).

There is therefore **no path from `&LoweredIr` to the string `"Column"`**. All leaf views
lower to `NodeKind::Primitive` and are mutually indistinguishable: `Column`, `Row`,
`Text`, `Button` and `TextField` differ only by an opaque integer whose meaning was
discarded.

### Gap B — the arena string table is empty (Gap G3)

`ArenaBuilder::finish()` yields an `IRArena` whose `string_table()` is the default, empty
table, so every `Value::Str(id)` prop emitted by lowering is a dangling id. This is
already recorded in `docs/adr/ADR-0033-flux018-string-table-gap.md` and deferred to a follow-up.
Codegen is one of the consumers that ADR names as affected: string *content*
(`Text("Hello")`, `Button(text: "Increment")`) cannot be rendered.

### Evidence

Driving the real pipeline (`parse` → `type_check` → `lower`) over

```flux
component Counter { state count: Int = 0 Column(spacing: 8) { Text("hi") Button(text: "inc") } }
```

yields:

```
arena.len() = 4
string_table.len() = 0
node kind=Primitive  component_id=3  props=[(0, Str(0))]
node kind=Primitive  component_id=4  props=[(20030, Str(1))]
node kind=Primitive  component_id=2  props=[(43518, Int(8))]
node kind=Component  component_id=1  props=[]
instances = 0
```

The four nodes are `Text`, `Button`, `Column` and `Counter` respectively, but nothing in
`LoweredIr` records that. `Str(0)` / `Str(1)` are `"hi"` / `"inc"`, unresolvable against
the empty table.

## Decision Drivers

- BR-002: a Kotlin/Compose developer unfamiliar with `.flux` must be able to read the
  generated output. Emitting `Primitive2(prop43518 = 8)` fails this outright.
- FLUX-021 §2.4 requires substring assertions on `Column(spacing = `, `items(`, `NavHost`
  — none of which can be produced from an opaque `ComponentId`.
- AGENTS.md §3.1 (boundary contract): the codegen agent owns
  `/crates/flux-codegen-kotlin/src/**` only, and must not make public-API changes to a
  sibling crate that other agents consume.
- `crates/flux-ir` had **uncommitted in-flight changes from another agent** at the time of
  writing, so editing it would risk stomping concurrent work
  (`flux-monorepo-verify` skill, Principle 2).

## Considered Options

**Option A — Reconstruct names inside the Kotlin codegen crate.**
Re-parse and re-type-check the source inside `codegen`, re-deriving the name → id mapping
by replaying lowering's interning order.
*Rejected:* duplicates lowering logic in a second crate, silently couples codegen to
lowering's private interning order, and breaks the moment lowering's traversal changes.
It also contradicts the given signature, which takes only `&LoweredIr`.

**Option B — Emit placeholder names (`Primitive2`, `prop43518`).**
*Rejected:* violates BR-002 and produces Kotlin that does not compile. Shipping snapshots
of unreadable, non-compiling output would make FLUX-021 green while delivering nothing.

**Option C — Fix it at source in `flux-ir` (Gap G3 closure, "FLUX-018b").**
Populate the arena string table and expose the component-name mapping, then implement
codegen against the corrected shape.
*Preferred, but out of this agent's ownership* and unsafe to apply while `flux-ir` has
another agent's uncommitted work in the tree.

## Decision Outcome

**Chosen: Option C, deferred to the `flux-ir` owner; FLUX-021 is reported BLOCKED rather
than implemented against an unusable input.**

No Kotlin codegen is implemented until `LoweredIr` can answer *"what is this node?"*.
Writing it against Gap A would mean either duplicating lowering (Option A) or emitting
unreadable output (Option B); both are worse than an honest block.

### Required unblocking change (`flux-ir` owner / FLUX-018b)

1. Add `ArenaBuilder::intern_string(&mut self, text: &str) -> StringId` and move the
   builder's accumulated `StringTable` into the `IRArena` at `finish()`. This closes
   Gap G3 exactly as `ADR-0033-flux018-string-table-gap.md` already scopes it, and makes
   `Value::Str` props resolvable via `arena.string_table()`.
2. Intern every component/primitive name during lowering and make it recoverable from
   `LoweredIr`, either by:
   - (a) storing a `ComponentId → StringId` map on `LoweredIr` / `IRArena`, or
   - (b) defining `ComponentId` to *be* the name's `StringId`, so
     `arena.string_table().resolve(node.component_id())` yields `"Column"` directly.

   Option (b) matches the contract's stated design (line 699: the host `AdapterRegistry`
   resolves `ComponentId` through the string table), so the host runtime needs this fix
   regardless of codegen.

Once either shape lands, codegen resolves `node.component_id()` to a name and the full
FLUX-021 mapping (§2.2), determinism (§2.3) and snapshot suite (§2.4) become implementable
as specified, with no change to the published `codegen(&LoweredIr) -> String` signature.

## Consequences

- **Bad (accepted):** FLUX-021 and FLUX-020 remain unimplemented until FLUX-018b lands.
  Phase 4 codegen is serialised behind a one-method `flux-ir` change.
- **Good:** no duplicated lowering logic, no unreadable snapshots locked into `insta`, and
  no cross-agent stomp of in-flight `flux-ir` work.
- **Good:** the required fix is small, already half-scoped by an existing ADR, and is
  needed by the host runtime's `AdapterRegistry` independently of codegen — so it is not
  codegen-specific scope creep.

## Open question for the orchestrator

An in-flight `flux-ir` lowering change comments that non-UI expressions (`let`, `onMount`,
`effect`, `provide`, `useContext`, `resource`, `createRef`) contribute no IR node because
*"codegen reads them from the AST directly."* That implies codegen may be expected to
consume the `Ast`/`TypedAST` **in addition to** `LoweredIr` — which conflicts with the
`codegen(lowered: &LoweredIr) -> String` signature mandated by FLUX-020/FLUX-021. The
orchestrator should settle the codegen input contract (IR-only vs IR + typed AST) before
either codegen backend is built, since it changes both crates' public API.

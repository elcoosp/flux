# ADR-0050: `ForEach` iteration / list-comprehension syntax

- **Status:** Accepted
- **Date:** 2026-08-29
- **Supersedes / related:** PRD-S (deferred list-comprehension), FLUX-051,
  ADR-0029 (frozen indentation grammar), FLUX-014 (keyed reconciliation)

## Context

Rendering a list of items needs iteration syntax. PRD-S deferred it as
ADR-gated: landing a `for`/comprehension construct risks disturbing the grammar
freeze PRD-L established (and ADR-0029's indentation-delimited surface). The
iteration design must also respect FLUX-014 — keyed items are reconciled at
runtime by the host, so the lowered IR carries an intentionally-empty body.

The production implementation already landed ahead of this document (parser
production, `ExprKind::ForEach`, type-checker rule, IR lowering, both codegen
backends, and parity goldens `foreach_grow` exist and pass in
`crates/flux-parity`). This ADR records the *decided* design so the feature has
its paper trail and FLUX-051 can close via the ADR → production → close flow.

## Decision

Adopt a single declarative `ForEach` view expression (no separate `for` loop or
comprehension syntax — the keyed-list repeater is the only iteration form in
MLP):

```flux
ForEach(items, key: fn(item) { item.id }) { item =>
    Row {
        Text text: item.name
    }
}
```

- **Grammar (ADR-0029, indentation-delimited):** `ForEach` is a view expression.
  The first argument is the collection (a `List`); `key:` is an optional lambda
  extracting a stable key per item (defaults to the item index); the block is the
  per-item body with a single binding (`item =>`).
- **Type checking:** the collection must be a `List<T>`; the body is checked with
  the item bound to type `T`. A non-`List` first argument is a type error
  (`ForEach expects a List, got …`).
- **Lowering / IR:** lowered to a `NodeKind::ForEach` node carrying the
  collection expression and key extractor (canonical form). Per FLUX-014 the body
  is emitted **empty** — keyed items are spliced by the host at runtime, so
  parity asserts an empty `ForEach` body across dev/swift/kotlin.
- **Codegen:** Swift emits `ForEach(<coll>, id: <key>) { item in … }`;
  Kotlin emits the `items { item -> … }` equivalent. Both render the empty-body
  wrapper.
- **No new opcodes / wire fields.** Iteration is a view-level construct lowered
  to the existing `ForEach` node; the Appendix E ISA is untouched (ADR-gated
  additions require a separate ADR).

## Consequences

- Lists render through one declarative primitive, reconciled key-by-key at
  runtime (scroll position / text / state preserved across diffs).
- The indentation grammar stays frozen; `ForEach` follows the same
  brace-free, layout-delimited surface as every other view expression.
- Adding another iteration form (e.g. a numeric `for i in 0..n`) is a new ADR,
  not a patch to this one.

## Verification

- `crates/flux-parity/src/sources.rs` fixtures `B34_LIFECYCLE` and
  `B36_ASYNC` both exercise `ForEach`; `cargo nextest -p flux-parity` asserts
  structural dev/swift/kotlin parity (39/39 pass).
- `foreach_grow` trace goldens (`crates/flux-parity/tests/trace-goldens/`)
  pin the release-path trace for phase-3 growth, gated by
  `phase3_foreach_grow_is_oq3_gated`.

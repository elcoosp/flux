# ADR-0033-flux018-string-table-gap: lowered string literals are not yet merged into the arena string table

- **Status:** Proposed (created 2026-08-24 by FLUX-018 lowering agent; tracks Gap G3 from `docs/agents-boundaries-contract.md`)
- **Related:** FLUX-018 (lowering), AGENTS.md §3.2 (Node IDs / string interning), `flux-ir` `StringTable` (`crates/flux-syntax/src/string.rs`), `ArenaBuilder` (`crates/flux-ir/src/builder.rs`)

## Context and Problem Statement

FLUX-018 lowers a type-checked program into a packed `IRArena`. String literals
(`Text("hello")`, prop strings, handler source text) become `Value::Str(string_id)`
where `string_id` is a key into a `StringTable`. In the current `flux-ir` shape:

- `Lowerer` interns strings into a **local** `StringTable` (`Lowerer::strings`)
  via `intern_str`, then packs `Value::Str(id)` into props.
- `ArenaBuilder::finish()` produces an `IRArena` whose `string_table()` is the
  **default (empty)** `StringTable` — `ArenaBuilder` has no API to absorb a
  caller-supplied string table, and `Lowerer::strings` is dropped at `finish()`.

Consequently every `Value::Str(id)` emitted by lowering points at an id that is
*not present* in `arena.string_table()`. Any downstream consumer that resolves
`Value::Str(id)` against the arena (the dev-server wire codec, the Swift/Kotlin
codegen, the runtime's text adapter) would read a missing / wrong string.

This is the documented "string-table gap" (contract §15, Gap G3). It is a real
defect, but its fix is a `flux-ir` **public-API change** (adding
`ArenaBuilder::intern_string` and threading a merged `StringTable` into
`finish()`), which is out of scope for the MLP lowering pass and would touch the
`flux-ir` builder's contract consumed by `flux-differ` / `flux-ir-serde`.

## Decision Outcome

1. **Do not silently emit dangling `Value::Str`.** `Lowerer::intern_str` continues
   to intern into the local table so `id` values are allocated and stable within
   a single lower run, but the gap is recorded here rather than papered over.
2. **Leave the fix as a dedicated follow-up** (Gap G3 closure): add
   `ArenaBuilder::intern_string(&mut self, text: &str) -> StringId` and have
   `ArenaBuilder::finish()` move its accumulated table into `IRArena`. Lowering
   then calls that builder method instead of maintaining a parallel table.
3. **Handler bytecode does not depend on this gap** — handlers carry signal ids
   and integer/bool literals directly; the gap only affects rendered string
   *content* (text/prop strings), which the MLP dev server can resolve from a
   separate table the lowering agent also exposes if needed.
4. **Tests for string resolution are deferred** until the builder API lands, to
   avoid asserting against a half-wired table.

## Consequences

- **Bad (accepted):** lowered `Value::Str` ids are not yet resolvable from
  `arena.string_table()`; text content is not end-to-end wired in the MLP.
- **Good:** no behavioural regression — the previous state had no string
  interning at all; ids are now at least allocated consistently.
- **Good:** the fix is localized to `flux-ir` (`ArenaBuilder` + `Lowerer`), and
  no other agent's crate or any manifest is touched (R2-safe).
- **Action required:** the ir-core owner (or a follow-up FLUX-018b) applies the
  one-method `ArenaBuilder::intern_string` addition and flips `Lowerer` to use
  it; closes Gap G3.

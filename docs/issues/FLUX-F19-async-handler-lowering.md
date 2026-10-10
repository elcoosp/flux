# FLUX-F19 — `await` in a handler is rejected by IR lowering (codegen's async path unreachable)

**Status:** CLOSED (round 56, 2026-10-10) — fixed in `bytecode.rs` with an end-to-end regression test

## Resolution (round 56)

The gap was one missing arm: `compile_block_or_expr` had no case for a bare
`ExprKind::Await(_)` statement. `let x = await …` already worked because it
routes through `compile_value`, but a handler whose *only* content was
`{ await Auth.login("u") }` failed with "unsupported handler expression" —
making the codegen async path (which renders the call as `.await()` /
`Task { }`) unreachable.

Fix: added

```rust
ExprKind::Await(_) => { let _ = self.compile_value(expr)?; Ok(()) }
```

to `crates/flux-ir/src/lower/bytecode.rs::compile_block_or_expr`. The
existing `compile_value` Await arm emits the AWAIT opcode sequence.

Regression tests:

- `crates/flux-codegen-kotlin/tests/handler_calls.rs::awaited_handler_renders_await_expression`
- `crates/flux-codegen-swift/tests/handler_calls.rs::awaited_handler_renders_await_expression`

---

## Original filing — for context

**Found:** round-34 bug hunt
**Severity:** medium (the documented first-class `await` handler contract in
ADR-0044/ADR-0045 cannot be reached end-to-end today)

## Summary

`flux-codegen-kotlin` and `flux-codegen-swift` both have code that renders an
`await <expr>` statement into native source:

* `Kotlin::render_await` → `{expr}.await()`
* `Swift::render_await` → `await {expr}`
* both `button_open` wrappers detect `await` in a handler body and wrap it in
  a coroutine (`GlobalScope.launch { … }` / `Task { … }`).

But the IR lowering (`crates/flux-ir/src/lower/bytecode.rs`) rejects any
handler whose body contains an `ExprKind::Await` node with

    Lower { message: "unsupported handler expression" }

so the codegen async path is **unreachable**: the very Flux program that
ADR-0044 demonstrates (`onPress: { await Auth.login("u") }`) fails before
codegen runs.

## What a fix needs

* Extend the handler-bytecode compiler in `bytecode.rs` to lower
  `ExprKind::Await(inner)` into the AWAIT / RESUME opcodes defined by
  ADR-0044 (`raw::AWAIT` / `raw::RESUME`), i.e. emit
  `<compile inner into a temporary reg>; AWAIT temp_reg; <bind result>`
  rather than bail.
* Mirror the lowering on the dev-path VM (already done — the Rust oracle
  has `future_reg` per ADR-0045).
* Add a codegen test that round-trips `onPress: { await Auth.login("u") }`
  through lowering + codegen and asserts `Auth.login("u").await()` (Kotlin)
  / `await Auth.login("u")` (Swift) appear inside a coroutine wrapper.
* Re-enable the currently-skipped test cases in
  `crates/flux-codegen-{kotlin,swift}/tests/handler_calls.rs`.

## Pre-existing codegen fixes landed in the same round

Round 34 fixed the codegen side of the same handler-body rendering (see
`fix(codegen-core): render bare and awaited calls in handlers (no placeholder)`):

* `render_stmt` now renders any `ExprKind::Call` through a new `render_call`
  helper, instead of collapsing every non-`RouterNav.navigate` call to
  `unsupported_placeholder`. This means a capability call
  (`onPress: { Auth.login("u") }`) now appears verbatim in generated source
  instead of `/* unsupported expr */ 0`.
* `render_stmt`'s `ExprKind::Await` branch now also routes the awaited
  expression through `render_call` (so the awaited call survives), instead of
  rendering its inner through `render_expr` (which falls back to
  `unsupported_placeholder` for any call).

Both fixes are necessary (but not sufficient, per the lowering gap above)
for the documented async handler contract to work end-to-end.

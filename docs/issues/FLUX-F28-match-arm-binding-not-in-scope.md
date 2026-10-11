# FLUX-F28 — match-arm pattern bindings are not in scope in the arm body

**Status:** OPEN (documented; not fixed this round)
**Found:** round-86 bug hunt (parity probes)
**Severity:** medium-high (a first-class `match` in a handler whose arm
binds a payload field and then uses that binding fails at IR lowering)

## Summary

`crates/flux-ir/src/lower/bytecode.rs::compile_match` emits a MATCH_TAG
test for each arm and jumps to the arm body, but **does not bind the
pattern's payload fields into a local register before compiling the
body**. When the arm body references a pattern-bound name, the compiler
falls through to `signal_of`, which fails with

    Lower { message: "`x` is not a state signal in scope" }

because the bound `x` is not a state signal — it is a payload field of
the ADT variant.

## Reproduction

    type Op =
      | Add(Int)
      | Sub(Int)

    compo C
      state n: Int = 0
      Button(text: "b", onPress: {
        let op = Add(1)
        match op {
          Add(x) => n = n + x
          Sub(x) => n = n - x
        }
      })

`compile` fails with `\`x\` is not a state signal in scope` at the
first `x` reference in the arm body.

## What a fix needs

1. In `compile_match`, **before** `compile_block_or_expr(&arm.body)`,
   allocate a register for each `Pattern::Ident` in the arm's field list
   and emit the corresponding `GET_FIELD` from the scrutinee's record.
   Push the binding into `self.locals` so subsequent `Ident` lookups
   resolve to the arm-local register, and pop the bindings after the
   arm body (or restore a saved locals snapshot).

2. The `Emitter` currently keys `locals` by `String` → `u8` register.
   `compile_match` must save the pre-arm locals, install the arm
   bindings, compile the body, then restore. A simple clone-and-restore
   is O(locals) per arm and fine at this scale.

3. The codegen paths (`render_stmt` for Swift/Kotlin) already thread the
   binding name through the emit but the dev-side IR lowering is what
   blocks; the fix is a `bytecode.rs` change only.

4. Add an end-to-end test: the `Op` example above must lower, compile,
   and run (matching the reference VM's `MATCH_TAG` / `GET_FIELD`
   behavior).

## Regression test

`crates/flux-parity/tests/parity.rs::match_arm_binding_is_a_known_gap`
is `#[ignore]`d and documents the current failure. When the fix lands,
un-ignore and assert parity.

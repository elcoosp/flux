# FLUX-F29 — Assignment to a lambda parameter produces a misleading error

**Status:** OPEN (documented; not fixed this round)
**Found:** round-86 bug hunt (handler probes)
**Severity:** low (the type checker accepts `fn(x) { x = 1 }`, but IR
lowering rejects it with "`x` is not a state signal in scope" — a real
user-facing inconsistency, though assigning to a lambda parameter is
probably itself a user error)

## Summary

`crates/flux-ir/src/lower/bytecode.rs::compile_assignment` routes every
`Ident` assignment through `signal_of`, which only knows state signals.
A lambda parameter, `let` binding, or match-arm binding is *not* a state
signal, so `x = 1` inside a lambda produces

    Lower { message: "`x` is not a state signal in scope" }

even though the *type checker* accepted the same expression (no error at
that layer). The same class as F28 (reads of match-arm bindings), but on
the write side.

The read path (`compile_value`'s `Ident` arm) already knows to check
`self.locals` before falling back to `signal_of`, so `x + 1` inside a
lambda works — the asymmetry (read OK, write fails) is what makes the
error confusing.

## Reproduction

    compo C
      Button(text: "b", onPress: fn(x) { x = 1 })

`compile` fails with `\`x\` is not a state signal in scope`. Removing
the assignment (probe `fn(x) { x + 1 }`) compiles fine; the identical
read against a lambda param is what makes the writer's error
misleading.

## What a fix needs

Decide the semantics, then implement:

* **If lambda parameters are meant to be mutable locals** (i.e. the
  language wants `fn(x) { x = 1 }` to write to the register holding
  `x`), `compile_assignment` must check `self.locals` first (mirroring
  the `compile_value` `Ident` path) and emit a register-to-register move
  into the local's slot. The name is then not required to be a state
  signal.

* **If lambda parameters are meant to be immutable** (the more common
  design for a functional/reactive language), the type checker should
  reject `x = 1` with a proper diagnostic ("cannot assign to a lambda
  parameter") *before* lowering, and `compile_assignment`'s error should
  say the same thing.

Either way, the current message is misleading because it implies the
name is unbound, when in fact it *is* bound — just not as a signal.

## Regression test

`crates/flux-parity/tests/parity.rs::assign_to_lambda_param_is_a_known_gap`
is `#[ignore]`d and documents the current failure. When the design is
chosen and implemented, un-ignore and assert either compilation or the
new diagnostic.

## Related

* `docs/issues/FLUX-F28-match-arm-binding-not-in-scope.md` — reads of
  match-arm bindings; same root cause (`signal_of` only knows signals,
  not the `locals` map).

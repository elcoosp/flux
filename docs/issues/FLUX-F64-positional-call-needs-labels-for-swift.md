# FLUX-F64 — A positional Flux call to a Swift function emits `f(1)` without the required label

**Status:** OPEN (documented; not fixed — needs callee's param names at
the emit site, which is a larger change)
**Found:** round-110 (the sibling of F63)
**Severity:** medium (a handler that calls a user function with a
positional argument emits invalid Swift; the function declaration
carries labels that Swift requires at every call site.)

## Summary

Companion of F63. F63 fixed the case where the Flux caller *wrote* a
named arg (`f(n: 1)`) and the emitter dropped the name. F64 covers the
sibling case: the Flux caller writes a positional arg (`f(1)`), and the
emitter does not synthesise the Swift-required label from the callee's
parameter list.

Given:

    fn f(x: Int) -> Int { x }

    compo C
      Button(text: "x", onPress: { f(1) })

The emitted Swift is:

    Button(action: { f(1) }) {
        Text("x")
    }

But `f` is `func f(x: Int) -> Int` — Swift requires `f(x: 1)`. The
output does not compile.

## Reproduction

`crates/flux-codegen-swift/tests/handler_named_args.rs::positional_arg_call_in_handler_emits_bare_positional_today`
(documented current behavior).

## What a fix needs

The emitter's `render_call` must, for a `Call` whose callee names a
user-defined function, look up that function's parameter list from the
AST and render each positional arg with its corresponding parameter
name as a label (Swift) / named arg (Kotlin). The pieces are already in
place:

* `Bridge` carries the AST.
* `flux_parser::FnDecl.params` lists the parameter names.
* `render_call` currently receives only `callee` and `args`; a small
  signature change to accept the callee's parameter-name list (or a
  `&Bridge` for the lookup) suffices.

Then `render_stmt`/`render_await`/`render_call` in
`crates/flux-codegen-core/src/expressions.rs` pass that list down.

## Impact

Every handler that calls a user function positionally emits a Swift
function call without the label the Swift declaration requires. Since
Flux allows positional args, this is a silent "generates invalid
Swift" path.

## Related

* **F63** — the sibling case; landed in the same round.
* The two together mean every call from a handler to a named-parameter
  Flux function is potentially broken on Swift.

# FLUX-F63 — Handler-call rendering drops named-argument names

**Status:** OPEN (fix landed in same commit)
**Found:** round-110 bug hunt (`flux-codegen-core/src/expressions.rs::render_call`)
**Severity:** medium (a Flux `handler` body calling a user function with
a named argument — `onPress: { f(n: 1) }` — emits `f(1)` in Swift /
Kotlin. Swift requires the label, so `f(n: Int) -> Int` is called as
`f(1)` which is a compile error. Kotlin tolerates positional calls to
functions with named-only parameters, but the user's intent is lost.)

## Summary

`crates/flux-codegen-core/src/expressions.rs::render_call`:

    fn render_call<B: Backend>(callee: &Expr, args: &[flux_parser::Arg]) -> String {
        let callee_text = /* … */;
        let arg_texts: Vec<String> = args.iter().map(|a| render_expr::<B>(a.value())).collect();
        format!("{}({})", callee_text, arg_texts.join(", "))
    }

`a.value()` drops the arg's name for a `Named` arg. Given a Flux handler
`onPress: { f(n: 1) }`, the emitted Swift is:

    Button(action: { f(1) }) {
        Text("x")
    }

But the Flux `fn f(n: Int)` declares `n` as a parameter name, so the
Swift side needs `f(n: 1)`. Emitted Kotlin is similarly
`f(1)` — Kotlin accepts positional calls to a function declared
`fun f(n: Int)` (parameter names in Kotlin are not required labels), so
Kotlin compiles; Swift does not.

## Reproduction

Confirmed by direct Swift codegen (probe_r110d):

    let src = "fn f(n: Int) -> Int { n }\n\ncompo C\n  Button(text: \"x\", onPress: { f(n: 1) })";
    // … lower, codegen to Swift …
    // emitted: Button(action: { f(1) }) { … }
    // swift.contains("f(n: 1)") == false
    // swift.contains("f(1)")   == true

## Impact

Any handler that calls a user function with a named argument emits
invalid Swift. Named-arg calls are the canonical form in Flux (they
survive at the surface grammar); users hit this the moment they try to
pass an argument whose name differs from the callee's parameter order.

## Fix

`render_call` should preserve named args, using the backend's
`named_arg` helper (the same one `render_args` already uses):

    let arg_texts: Vec<String> = args.iter().map(|a| match a {
        flux_parser::Arg::Named { name, value } => {
            B::named_arg(&name.name, &render_expr::<B>(value))
        }
        flux_parser::Arg::Positional(value) => render_expr::<B>(value),
        _ => B::unsupported_placeholder(),
    }).collect();

`B::named_arg` already emits the correct per-language shape
(`name: value` on Swift, `name = value` on Kotlin).

## Regression tests

`crates/flux-codegen-swift/tests/handler_named_args.rs` (added in this
commit):

* `named_arg_call_in_handler_preserves_label` — asserts the emitted
  Swift contains `f(n: 1)`.
* `positional_arg_call_in_handler_is_unchanged` — positive control.

`crates/flux-codegen-kotlin/tests/handler_named_args.rs`:

* `named_arg_call_in_handler_preserves_name` — asserts the emitted
  Kotlin contains `f(n = 1)`.

## Related

`crates/flux-parity/src/recognize_*` — the recognizer does not look at
handler bodies, so parity did not catch this. Handler-body correctness
is a codegen-side check, not a parity check.

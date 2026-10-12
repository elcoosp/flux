# FLUX-F65 — Nested call inside an outer call collapses to the unsupported placeholder

**Status:** OPEN (fix landed in same commit)
**Found:** round-111 bug hunt (`flux-codegen-core/src/expressions.rs`)
**Severity:** medium (a handler like `onPress: { f(g(1)) }` — a call whose
argument is another call — emits `f(0 /* unsupported */)` in Swift/Kotlin.
The inner call is silently replaced with the placeholder; the outer call
survives with the wrong argument. Same class as F63: a codegen-side
silent loss that parity does not catch.)

## Summary

`crates/flux-codegen-core/src/expressions.rs::render_expr` handles a
fixed set of expression kinds and falls back to `B::unsupported_placeholder()`
for everything else (calls, records, lambdas):

    pub(crate) fn render_expr<B: Backend>(expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Int(…) | ExprKind::Float(…) | … => { … }
            ExprKind::Binary { … } => { … }
            ExprKind::Field { … } => { … }
            ExprKind::List(…) => { … }
            _ => B::unsupported_placeholder(),   // <- Call lands here
        }
    }

`render_call` (the call-aware path used by `render_stmt`) does not route
the *arguments* through itself; it calls `render_expr` for each arg:

    let arg_texts: Vec<String> = args.iter().map(|a| match a {
        flux_parser::Arg::Named { name, value } => B::named_arg(&name.name, &render_expr::<B>(value)),
        flux_parser::Arg::Positional(value) => render_expr::<B>(value),
        _ => B::unsupported_placeholder(),
    }).collect();

When an arg is itself a call, `render_expr` falls through to the
placeholder. The outer call renders correctly, but the inner call is
lost.

## Reproduction

`crates/flux-codegen-swift/tests/probe_r111c.rs` (deleted after the
probe run) — a handler with `f(g(1))`:

    fn g(n: Int) -> Int { n }
    fn f(n: Int) -> Int { g(n) }

    compo C
      Button(text: "x", onPress: { f(g(1)) })

Emitted Swift:

    Button(action: { f(0 /* unsupported */) }) {
        Text("x")
    }

The literal `g(1)` is gone; the argument is the placeholder. Same for
deeper nesting.

## Impact

Any handler whose call passes another call as an argument (or a list
literal containing a call, or a record field whose value is a call —
all of which route through `render_expr`) silently loses the inner
value. This is exactly the class of bug the `_ => B::unsupported_placeholder()`
fallback exists to surface — but here it surfaced *and* the resulting
code is silently broken because the placeholder (`0 /* unsupported */`)
does not visually read as "the value you wrote is gone" the moment the
programmer is inside a working call.

## Fix

`render_expr` should route `Call` through `render_call`:

    ExprKind::Call { callee, args, .. } => render_call::<B>(callee, args),

That makes the recursion mutual (`render_expr` → `render_call` →
`render_expr`), but termination is guaranteed by the AST being finite
and the pattern matching reducing the depth of the visited expression.

## Regression tests

`crates/flux-codegen-swift/tests/handler_nested_calls.rs` (added in
this commit):

* `nested_call_in_handler_preserves_inner_call` — asserts the emitted
  Swift contains `f(g(1))` (with the appropriate labels).

## Related

* **F63** — sibling class (named-arg label dropped).
* **F64** — positional call label missing on Swift.
* All three are codegen-side silent loss on handler bodies.

# FLUX-F26 — `batch { … }` / `untrack { … }` in a handler are rejected by IR lowering

**Status:** OPEN (documented; no fix landed this round)
**Found:** round-86 bug hunt (parity probes)
**Severity:** medium (the first-class `batch`/`untrack` constructs from the
spec cannot appear inside a handler; any Flux program that writes
`onPress: { batch { … } }` fails at lowering with "unsupported handler
expression")

## Summary

`batch { … }` and `untrack { … }` parse as
`ExprKind::Lifecycle { kind: LifecycleKind::Batch | Untrack, body }`
(`crates/flux-parser/src/ast/expr/expr_kind.rs:171`). The handler-bytecode
compiler in `crates/flux-ir/src/lower/bytecode.rs::compile_expr_stmt` has
no `ExprKind::Lifecycle` arm, so any handler containing them falls through
to the final `_ =>` and errors out with

    Lower { message: "unsupported handler expression" }

before codegen ever runs. This is the same class as F19 (which was about
`ExprKind::Await`); the `Await` arm was added, but `Lifecycle` was not.

## Reproduction

    compo C
      state n: Int = 0
      Button(text: "x", onPress: { batch { n = n + 1 } })

`compile` returns:

    ParityPipelineError("lower: lowering error at Span { … }: unsupported
    handler expression")

`untrack { … }` fails identically.

## What a fix needs

1. **IR lowering** (`bytecode.rs::compile_expr_stmt`): add
   `ExprKind::Lifecycle { kind, body }` arm that:
   * for `Batch` / `Untrack`, lowers the body as a plain block (both are
     reactive-scheduler hints, semantically transparent to the reference
     VM — its signal writes are already batched at the run level);
   * for the other `LifecycleKind`s (`OnMount` / `OnCleanup` / `Effect` /
     `Derived`), keeps the current "unsupported handler expression" error
     — they are block items, not handler-body statements.

2. **Codegen** (`flux-codegen-core/src/expressions.rs::render_stmt`):
   `Lifecycle { kind: Batch | Untrack, body }` in a handler should emit
   the body as an inline block, matching the IR-lowering semantics. The
   current `render_stmt` has no `Lifecycle` arm, so even after the IR
   fix the emitter would fall through to
   `render_expr::<B>` → `unsupported_placeholder` and emit
   `0 /* unsupported */`.

3. **Parity regression test**: `onPress: { batch { n = n + 1 } }` must be
   equivalent across dev/Swift/Kotlin.

## Why this is filed, not fixed in this round

The fix touches both the IR lowerer and the codegen and requires deciding
what the release-side semantics of `batch`/`untrack` are (Kotlin and
Swift have no direct equivalents — the honest translation is "run the
block, signal writes go through the normal path"; whether the host
scheduler treats the group as a batch is a separate question). Each is
its own bug surface and the round is already wide.

## Regression test

`crates/flux-parity/tests/parity.rs::batch_inside_handler_is_a_known_gap`
is `#[ignore]`d and documents the current "unsupported handler expression"
failure. When the fix lands, un-ignore and assert parity.

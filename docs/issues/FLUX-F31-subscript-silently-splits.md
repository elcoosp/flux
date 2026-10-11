# FLUX-F31 — `xs[0]` is silently parsed as two sibling expressions, not an index

**Status:** OPEN (documented; not fixed this round)
**Found:** round-86 bug hunt (list-access probes)
**Severity:** medium (a user writing `state first: Int = xs[0]` gets
two separate items — `state first = xs` and a stray list literal `[0]`
— silently. The type checker then rejects the first with a confusing
message about the list, and the stray list literal either compiles to a
dead expression or triggers a lowering error, depending on context.)

## Summary

The parser's `postfix_expr` loop handles `.`, `?.`, `(`, and `{` — but
**not `[`**. When the primary expression is followed by `[`, the loop
breaks, the state initializer is parsed as just the identifier, and the
next token sequence is parsed as the next block item (a list literal).

For the input

    compo C
      state xs: List[Int] = [1, 2]
      state first: Int = xs[0]
      Text("x")

the AST is:

    state xs = List([Int(1), Int(2)])
    state first = Ident("xs")            // <-- silently truncated
    expr List([Int(0)])                  // <-- a stray list literal
    expr Text("x")

The type checker then errors with `expected Int, got List[Int]` on the
`state first = xs` line — a misleading message about a valid expression.

## Reproduction

    compo C
      state xs: List[Int] = [1, 2]
      state first: Int = xs[0]
      Text("x")

`flux_parity::compile` returns:

    Type { message: "expected `Int`, got `List[Int]`" }

Same shape for `xs[0]` in a handler: the enclosing handler is parsed as
two statements, the second being a bare list literal, which then fails
at IR lowering with "unsupported handler expression".

## What a fix needs

The parser must recognize `[` after a primary expression as a subscript.
This is a three-surface decision:

1. **Parser.** In `postfix_expr`, add an `TokenKind::LBracket` arm that
   parses `expr '[' idx ']'` into an `ExprKind::Index { base, index }`
   (a new AST variant). Bump any span/lint expectations.

2. **Type checker.** Add an `infer` arm for `Index { base, index }`
   that:
   * checks `base` is a `List[T]` (or a `String`, if string indexing
     is intended);
   * checks `index` is an `Int`;
   * returns `T`.

3. **IR lowering + codegen.** Add `Index` to `compile_value` (it
   lowers naturally to `LIST_GET` for lists — the opcode already
   exists and is exercised by the type-checker test corpus), and to
   the codegen `render_expr`.

Alternatively, if subscript syntax is intentionally unsupported, the
parser should **reject `[` in postfix position with a clear error**
("subscript syntax is not supported; use `list.at(i)`") instead of
silently producing two expressions. That is a one-line change to the
postfix loop, and the error message tells the user what to do.

Given the round-86 scope, this issue is filed rather than fixed: the
"reject with a clear error" path is a small change, but the "support
subscripts" path is a multi-crate feature. Either is a dedicated
sub-round.

## Regression tests

Filed at `crates/flux-parity/tests/parity.rs`:
* `subscript_index_is_a_known_gap` — `#[ignore]`d, asserts the current
  "expected `Int`, got `List[Int]`" failure for `state first: Int =
  xs[0]`.

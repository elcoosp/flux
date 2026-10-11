# FLUX-F55 — `flux fmt` silently drops every user comment

**Status:** OPEN (documented; not fixed this round)
**Found:** round-98 bug hunt (`flux-parser/fmt`)
**Severity:** high (data loss: a user running `flux fmt` on a file
loses every `//` comment in the file, with no warning. The flux CLI
runs `flux fmt` on every file in a path, so the entire project's
comments are silently removed. Any TODO, doc note, or historical
context in `.flux` files is destroyed by one command.)

## Summary

`crates/flux-parser/src/fmt/mod.rs::format_ast` renders the AST back
to source. The parser does not attach comments to AST nodes — line
comments are consumed by the lexer and discarded — so the AST carries
no representation of them. `format_source` therefore emits a
comment-free canonical form of every input.

That is *correct* as a formatter given the AST-only pipeline, but it
is *destructive* as a tool. The `flux fmt` CLI writes the formatted
result back in place:

    std::fs::write(path, &formatted)?;

with no check that any comments existed in the original, and no
warning.

## Reproduction

`crates/flux-parser/tests/probe_r98d_comments.rs` (deleted after the
probe run; reproducible in three lines):

    let src = "compo C\n  // important note\n  Text(\"x\")\n";
    let out = format_source(src, 0, "f.flux").unwrap();
    assert!(out.contains("important note")); // fails

Every shape of comment is dropped:

* Line comments inside a component body.
* File-header comments (before `compo`).
* Inline trailing comments (`state n: Int = 0  // counter`).

## What a fix needs

The parser must be extended to *preserve* comments through the
pipeline. Concretely:

1. **Lex.** Line comments are already lexed (the lexer skips them, but
   they are recognizable). Emit them as a distinct `Token` kind, or
   keep a side table of `(span, text)`.
2. **AST.** Attach comments to their AST nodes (leading trivia on the
   next token, trailing trivia on the current). This is the classic
   "trivia-preserving parser" design — gofmt, rustfmt, prettier all
   do this.
3. **Format.** When rendering the AST back to source, emit the
   attached trivia in the canonical position.
4. **Ordering / blank lines.** Comments between declarations must
   also be preserved (they cannot attach to a single next-token if
   they precede a blank line).

This is a medium-size feature — not a bug fix — and touches the
lexer, the parser, the AST, and the printer. Filing rather than
fixing.

## Interim mitigation

The `flux fmt` CLI could:

* **Refuse to write** any file whose formatted output would drop
  comments. Detect via a cheap "does the source have more `//` than
  the formatted output" check, and error with a "comment-preserving
  formatting is not yet implemented" message.
* Or: require `--allow-comment-loss` to opt in.

The minimal ship-blocking mitigation is a **warning** on `flux fmt`
whenever it is about to write a file whose comment count dropped,
so the user can Ctrl-C before losing data.

## Regression test

`crates/flux-parser/tests/probe_r98d_comments.rs` (deleted after the
probe run) is the reproduction. A permanent `#[ignore]`d version
documenting the current behavior should be added once the fix shape
is chosen.

## Related

`docs/issues/FLUX-F53-nesting-prescan-counts-comment-brackets.md` —
the same family (comment handling has holes across the pipeline).

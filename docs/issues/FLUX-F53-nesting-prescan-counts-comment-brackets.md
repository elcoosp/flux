# FLUX-F53 — Nesting pre-scan counts `{` / `[` / `(` inside comments

**Status:** OPEN (fix landed in same commit)
**Found:** round-97 bug hunt (`flux-parser/parser.rs::check_nesting_depth`)
**Severity:** medium (a legitimate Flux source with a comment that
mentions more than 16 unmatched brackets — a TODO, a code example, a
link — is rejected with a spurious "nesting exceeds the maximum depth
of 16" before the actual parser ever runs. The rejected source is
otherwise valid Flux.)

## Summary

`check_nesting_depth` (the O(n) pre-scan that guards against deep
recursion in the recursive-descent parser) walks the source and
increments a depth counter on every `{` / `[` / `(`, decrements on the
matching closers, and rejects when the counter exceeds 16. The scan
skips characters inside `"…"` string literals (so `"{"` inside a
literal does not count), but **does not skip line comments**.

Any `//`-comment that contains more than 16 unmatched open brackets —
a TODO, a pasted code snippet, a URL with parens, an ASCII-art
diagram — trips the ceiling and rejects the whole file. The error's
location is inside the comment, and the hint ("extract the inner
expression or view into its own `compo`") makes no sense for a
comment.

## Reproduction

`crates/flux-parser/tests/probe_r97d.rs` (deleted after the probe run;
reproducible in two lines):

    compo C
      // { { { { { { { { { { { { { { { { { { { { {
      Text("x")

`flux_parser::parse` returns:

    nest …  // … nesting exceeds the maximum depth of 16 @ 2:22

Same shape for `[` / `(` and for a `//`-comment with 20 unmatched
brackets of any mix. A comment whose brackets are balanced (e.g.
`// a { b } c { d } e`) is accepted only because the counter nets out
at zero — the ceiling check happens *at the moment the 17th opener is
seen*, so a balanced comment with 17 openers on one line still trips
it.

## Fix

Skip `//`-line-comments in the pre-scan. When the scanner sees two
consecutive `/` outside a string literal, advance to the next `\n`
without counting anything:

    b'/' if next == b'/' => {
        for ch in chars.by_ref() {
            if ch.0 == b'\n' { break; }
        }
    }

(The scanner already has a `peekable` `char_indices` iterator, so the
lookahead is trivial.)

Flux has no block comments, so no additional handling is needed —
confirmed by `grep` on `crates/flux-parser/src/lexer.rs`.

## Regression tests

`crates/flux-parser/src/error.rs` (or a new tests file) — added in this
commit:
* `comment_with_unmatched_brackets_does_not_trip_nesting_guard`
* `comment_with_many_unmatched_parens_does_not_trip_nesting_guard`

Both write a comment containing 20 unmatched openers and assert the
source parses cleanly.

## Related

`docs/issues/FLUX-F51-caret-misalign-non-ascii.md` — the same pre-scan
family (comment/string handling in the parser's diagnostics). Both
files share a "best-effort lexical scan" assumption that has holes.

## Impact today

Any Flux file authored with a comment that mentions raw syntax (very
common in real code — readmes, TODOs, examples, links) is rejected at
parse time with a message pointing at the comment. Users see a
"nesting" error and cannot find the bug because there is none.

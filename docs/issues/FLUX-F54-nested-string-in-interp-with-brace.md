# FLUX-F54 — A nested string inside `{…}` interpolation cannot contain `{` or `}`

**Status:** OPEN (documented; not fixed — requires re-lexing or a grammar
change)
**Found:** round-97 bug hunt (`flux-parser/parser.rs::interp_expr`)
**Severity:** low (unusual code path: writing a string literal whose only
content is a brace, inside an interpolation. Realistic user programs
rarely do this. But the failure mode is *silent-looking* — the error is
"unterminated string literal" pointing at the outer string, and the
hint doesn't say "you cannot nest braces inside a nested string inside
an interpolation".)

## Summary

`crates/flux-parser/src/parser.rs::string_lit` calls `interp_expr`
whenever the outer string contains `{`. `interp_expr` walks the source
byte-by-byte, tracking `{`/`}` depth for the interpolation. Inside
that walk it has a nested-string skip:

    b'"' => {
        idx += 1;
        while idx < self.source.len() {
            match bytes[idx] {
                b'\\' => idx += 1,
                b'"' => break,
                _ => {}
            }
            idx += 1;
        }
    }

This is *supposed* to make `"foo"` a self-contained token inside the
interpolation. But the outer brace counter does not know that the `}`
inside the nested string is not the interpolation's terminator. So
for the string

    Text("{ "}" }")

the walk sees:

1. `{` → interp opens.
2. ` ` then `"` → nested-string skip begins.
3. Scans: `}` — not `\`, not `"`, so advance.
4. `"` — end nested string, break.
5. Outer loop continues past the nested-string close, then sees `"`...

The trace is ambiguous because the closing quote of the nested string
also looks like the opening quote of a new string to the outer loop.
The result is either "unterminated string" on the outer literal (the
observed failure), or a mis-tokenized interp; the parser does not
disambiguate.

## Reproduction

    compo C
      state s: String = "x"
      Text("{ "}" }")

`flux_parser::parse` fails with `unterminated string literal @ 3:16`.

The same shape with an empty inner string works:

    Text("{ "" }")   // parses

And an inner string without braces works:

    Text("{ "hi" }") // parses

Only the case where the inner string contains `{` or `}` fails.

## What a fix needs

The parser's `interp_expr` must distinguish (a) the `}` that closes the
interpolation from (b) a `}` inside a nested string literal. The
current code's nested-string skip is *inside* the brace-tracking loop
but the brace-tracking has already been advanced to `}` before the
skip branch runs — so the skip never suppresses the interp-close.

Options:

1. **Move the nested-string skip ahead of the brace counter** in the
   main loop. When the scanner sees `"` (unescaped), jump past the
   whole literal before re-checking for `}`.

2. **Re-lex the interpolation's source** as a fresh parser pass (the
   parser already does this for `interp_expr` — it constructs a
   sub-`Parser` and calls `.expr()`) and let that pass track the
   string; the outer brace counter is only needed to find the
   interpolation's end, and could be discarded once sub-parse
   succeeds.

Option 2 is more robust: `interp_expr` currently scans bytes to find
the matching `}` *before* invoking the sub-parser. If it instead tries
the sub-parse greedily and lets a parse failure determine "try a
longer slice", nested strings with braces are handled by the
sub-parser's own string scanner.

Neither is a one-line fix; both are a parser round of their own.

## Regression test

`crates/flux-parser/tests/nested_string_in_interp.rs` — added in this
commit. Asserts the current failure (`unterminated string literal`).
When the fix lands, flip the test to require successful parse.

## Related

`docs/issues/FLUX-F53-nesting-prescan-counts-comment-brackets.md` —
both are comment/string handling in the parser's lexical scanning.
`docs/issues/FLUX-F51-caret-misalign-non-ascii.md` — the same family.

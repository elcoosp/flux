# FLUX-F23 — `comment_spans` misclassifies quotes preceded by an escaped backslash

**Status:** OPEN (fix landed in same commit)
**Found:** round-86 bug hunt
**Severity:** low (a `//` comment after a string literal containing an
escaped backslash — e.g. `"C:\\path"` — is not highlighted as a comment;
no compilation/behaviour impact)

## Summary

`crates/flux-lsp/src/semantic_tokens.rs::comment_spans` toggles its
`in_string` flag on every `"` byte whose *immediate predecessor* is not a
backslash:

    b'"' => {
        if i == 0 || bytes[i - 1] != b'\\' {
            in_string = !in_string;
        }
        i += 1;
    }

That check is only correct when every backslash in the source is an
escape. It misfires on a literal ending in an escaped backslash — e.g.
`"C:\\"` — whose byte sequence is `"` `\` `\` `"`. At the closing `"`, the
byte immediately before it is `\`, so the toggle is skipped; `in_string`
stays `true` for the remainder of the file, and any following `//` comment
is never emitted as a `COMMENT` semantic token.

The same class of bug exists in the codegen recognizer
(`recognize_swift::tokenize::split_tokens` handles escapes correctly since
round-30; this instance was overlooked).

## Reproduction

    state path: String = "C:\\"
    // this comment should be highlighted

Before the fix, the `COMMENT` token count is 0; after, it is 1.

## Fix

Track escape state while scanning: walk the string literal byte by byte
with an `escaped` flag so a `"` toggles `in_string` iff it is not escaped
by an *odd* run of backslashes. This is the same rule the Swift recognizer
uses.

## Related

* `crates/flux-lsp/src/semantic_tokens.rs`
* `crates/flux-parity/src/tokenize.rs` (correct implementation from round 30)

# FLUX-F51 — `ParseError::render` mis-aligns the caret for non-ASCII source

**Status:** OPEN (documented; low severity; not fixed — the fix is
mechanical but touches the render contract)
**Found:** round-95 bug hunt (`flux-parser/error.rs`)
**Severity:** low (cosmetic; the caret is off by exactly the number
of bytes minus characters preceding the diagnostic column on that
line — 1 cell for a single multi-byte char, N cells for N)

## Summary

`crates/flux-parser/src/error.rs::ParseError::render` builds the
caret underline as:

    let caret_pad = " ".repeat(self.location.column.saturating_sub(1) as usize);

`location.column` is a **character** count (`Location::from_offset`
increments it once per `char`, and its doc says so). But the source
line printed just above the caret is a **byte** sequence. A terminal
renders each UTF-8 character as `char_width(c)` display cells, not
`char.len_utf8()` cells — so for source with a multi-byte character
before the diagnostic, the caret is off by `sum(len_utf8(c) -
display_width(c))` cells.

For the common case (`café` has one 2-byte `é` whose display width is
1), the caret is off by exactly 1 cell per multi-byte character
preceding it on the line.

## Reproduction

`flux-parser` test (added in this commit):

    let err = parse("compo C\n  Text(\"café x\") !", 0, "f.flux")
        .expect_err("trailing `!` is a parse error");
    // The rendered caret is 1 byte left of the intended `!`.
    let rendered = err.render();
    eprintln!("{rendered}");

The alignment is visible in the rendered block: the `^` underlines the
space before `!`, not `!` itself.

## What a fix needs

Two options:

1. **Pad by the byte offset of the diagnostic's column** — compute
   the byte offset of `location.column - 1` characters into
   `line_text`, and pad the caret by *that many bytes* minus the
   byte offset of the line start (always 0). For ASCII this is
   identical to the current code; for non-ASCII this aligns the
   caret to the byte position of the intended char, which is what a
   byte-oriented terminal renders.

   ```rust
   let byte_pad = self.line_text
       .char_indices()
       .nth((self.location.column.saturating_sub(1)) as usize)
       .map_or(self.line_text.len(), |(b, _)| b);
   let caret_pad = " ".repeat(byte_pad);

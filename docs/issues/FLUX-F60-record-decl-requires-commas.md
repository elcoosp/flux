# FLUX-F60 — `record R { … }` requires comma separators; multi-line layout form rejected

**Status:** OPEN (fix landed in same commit)
**Found:** round-106 bug hunt (`flux-parser/parser.rs::record_decl`)
**Severity:** medium (the multi-line `record R { x: Int, y: Int }` form
without commas between fields is rejected with `expected '}', found 'y'`.
The parser's own `skip_layout()` call indicates the multi-line form was
*intended* to be supported — indentation drives Flux structure
everywhere else — but the `record_decl` parser's `break` on a
non-comma makes the trailing-comma mandatory. This is inconsistent
with every other block-bodied declaration: `compo` bodies, `if`
branches, `ForEach` bodies all use layout separators, not commas.)

## Summary

`crates/flux-parser/src/parser.rs::record_decl`:

    self.eat(TokenKind::LBrace)?;
    let mut fields = Vec::new();
    self.skip_layout();
    while !self.at(TokenKind::RBrace) {
        let field_name = self.ident()?;
        self.eat(TokenKind::Colon)?;
        let ty = self.ty()?;
        fields.push(RecordField { … });
        self.skip_layout();
        if !self.try_eat(TokenKind::Comma) {
            break;                              // <-- HERE
        }
        self.skip_layout();
    }
    self.eat(TokenKind::RBrace)?;

The `break` at line 575 assumes the only fields-separator is a comma.
On a multi-line record like

    record Point {
      x: Int
      y: Int
    }

the parser reads `x: Int`, calls `skip_layout()`, sees the next token
is `y` (the next field's identifier), and `try_eat(Comma)` returns
`false` — the `break` fires, the loop exits, and `eat(RBrace)` fails
with `expected '}', found 'y'`.

## Reproduction

`crates/flux-parser/tests/probe_r106_record.rs` (deleted after the
probe run) — three shapes:

* **Works:** `record P { x: Int, y: Int }`
* **Works:** `record P {\n  x: Int,\n  y: Int,\n}` (comma-terminated)
* **Fails:** `record P {\n  x: Int\n  y: Int\n}` (no commas)

## Impact

Any user writing the multi-line form (which the `skip_layout()` calls
in `record_decl` clearly anticipate) is rejected. The single-line form
with commas works. Real Flux record declarations with more than a
couple of fields naturally get written multi-line, so this hits the
common case.

The type checker is also affected transitively: `type P = { … }` is a
separate declaration (`type_decl` + `record_decl` for the anonymous
form is not supported — see F51's neighbors), so only `record Name { … }`
is broken.

## Fix

The `break` should not fire when the next token starts another field.
Drop the comma requirement — `Comma` is optional between fields:

    self.skip_layout();
    let _ = self.try_eat(TokenKind::Comma);   // optional separator
    self.skip_layout();

The `while !self.at(RBrace)` guard at the top of the loop terminates
the loop cleanly when a `}` is reached. If the next token is neither a
`}`, a `,`, nor a valid field name (an `Ident`), `self.ident()` at the
top of the next iteration fails with the appropriate error message.

Alternative: accept both separators (comma and layout) explicitly with
a match on the next token — but the simple "comma is optional" change
covers every case that today's grammar supports without changing the
accepted language beyond adding the multi-line form.

## Regression tests

`crates/flux-parser/tests/record_decl_layout.rs` (added in this
commit):

* `multi_line_record_without_commas_parses` — the failing case.
* `single_line_record_with_commas_still_parses` — positive control.
* `multi_line_record_with_commas_still_parses` — positive control.
* `empty_record_parses` — edge case.

## Related

`docs/issues/FLUX-F51-caret-misalign-non-ascii.md` — the parser
grammar has multiple gaps; this is the first record-decl one filed.

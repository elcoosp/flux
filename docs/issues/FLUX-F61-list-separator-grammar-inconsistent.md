# FLUX-F61 — List-separator grammar is inconsistent across productions

**Status:** OPEN (documented; not fixed — the fix is a grammar decision)
**Found:** round-106 bug hunt (parser probes after the F60 record-decl fix)
**Severity:** low-medium (three list-like grammar productions disagree on
whether a comma separates elements, whether it is optional, and whether
it is forbidden. Users have to learn three rules where one would do.)

## Summary

Across the Flux surface grammar, list-like productions handle comma
separators three different ways:

| Production | Comma required? | Layout works? | Notes |
|---|---|---|---|
| `record R { a: T, b: T }` fields | **optional** (F60) | yes | comma optional; layout accepted |
| `compo Card\n  props { a: T\n b: T }` | **optional** | yes | comma optional; layout accepted |
| `compo Box[T, U]` generic params | not accepted | — | layout accepted; multi-line works |
| `trait T[X] { fn … }` method list | not accepted | — | layout accepted; multi-line works |
| `capability C { fn … }` method list | not accepted | — | layout accepted; multi-line works |
| `fn f(a: T, b: T) -> R` params | **required** | no | comma mandatory; multi-line without commas fails |
| `type C = | A | B` ADT variants | **forbidden** | — | a comma after a variant is a syntax error |

That last row is the more striking: `type Color =\n  | RGB(Int),\n  | HSL(Int),` is rejected with `expected a declaration, found ','`. The `|`-prefixed variant form is layout-separated only.

The `fn` parameter list is the odd man out on the "required" side: `fn f(\n  a: Int\n  b: Int\n)` fails with `expected ')', found 'b'`, while every *other* brace/paren-delimited list in the grammar accepts layout-separated elements.

## Reproduction

`crates/flux-parser/tests/probe_r106b.rs` (deleted after the probe run):

* `fn f(\n  a: Int\n  b: Int\n) -> Int { a }` → `expected ')'`, found `'b'`
* `type Color =\n  | RGB(Int),\n  | HSL(Int),\n` → `expected a declaration, found ','`

Both are rejected; the corresponding layout-separated forms without commas (for `fn`) and with `|` separators (for ADTs) parse fine.

## Impact

Two things:

1. **Surprising errors.** A user who just learned that `record { }` accepts
   layout-separated fields tries the same in a `fn` signature and gets
   `expected ')'`. The error message does not hint that a comma is
   required.

2. **Different mental models.** Flux documents itself as an indentation-
   significant language. Multi-line lists separated by layout should be
   the *norm*, and commas an *option*. `fn` breaks that norm; `type`
   inverts it (commas are errors).

## What a fix needs

A single grammar decision, applied uniformly:

**Option A — commas optional everywhere.** Layout-separated elements
work in every list; a comma is accepted but never required. This is
the most permissive and matches the language's indentation-significance
story. `fn` and `type` both change to accept the F60 shape.

**Option B — commas required everywhere.** Record fields, props, and
ADT variants all require commas; layout never separates. Reverses F60.
Predictable but verbose and contradicts the language's design.

**Option C — commas never accepted.** Every list is layout-separated;
commas are syntax errors. Matches Python. Reverses F60 in the other
direction and forbids the single-line `record R { x: Int, y: Int }` form.

Option A is the smallest change from today's state: `fn` gains layout
support, `type` gains comma tolerance. The parser edits are localized
to `fn_sig`/`param` and `type_decl`/`variant`.

## Regression tests

None added — the fix shape is a design call. Once the grammar decision
lands, a `parser_grammar_consistency.rs` test should assert the chosen
rule against every list-like production.

## Related

* **F60** — the record-decl case of this same class, just fixed.
* **F51** — caret misalignment, another parser-grammar-family issue.

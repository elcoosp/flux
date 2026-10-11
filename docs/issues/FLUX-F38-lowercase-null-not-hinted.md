# FLUX-F38 — Lowercase `null` / Swift `nil` give a confusing "unbound name" error

**Status:** OPEN (documented; not fixed this round)
**Found:** round-87 bug hunt (type-checker probes)
**Severity:** low (DX only: the message is correct but does not tell
the user the correct Flux spelling, and a Kotlin/Swift developer's
muscle memory is to write `null` / `nil`)

## Summary

The Flux spec's absent-value literal is `Null` (capital N, ADR-0051).
The parser recognises only that spelling — `flux-parser/src/parser.rs`
has

    } else if text == "Null" {
        // The `Null` literal (FLUX-053 / ADR-0051) — the absent value
        // inhabiting every `Option[T]`.

but nothing for the Kotlin-style `null` (lowercase) or the Swift-style
`nil`. Both are read as a plain identifier, so a user writing

    compo C
      state o: Option[Int] = null

gets

    type-check: unbound name `null` (declare it with `let`, `state`,
    or bring it into scope via `import`)

which is accurate but points the user at scope resolution when the real
issue is "you wanted the `Null` literal (capital N)".

The same happens for Swift-authored `nil`.

## What a fix needs

Either:

1. **Accept `null` / `nil` as aliases for `Null`.** One line in
   `parser.rs::primary`'s identifier branch. Most user-friendly, at the
   cost of two more names to reserve in the codegen `reserved.rs`
   table (they are already there — see below).

2. **Add a hint.** When the type checker sees an unbound name matching
   `null` or `nil`, add
   `.with_hint("the Flux `Null` literal is spelled with a capital N")`.

3. **Both.**

The `reserved.rs` table already includes `null` (Kotlin's reserved
word), so a Flux author can *not* declare a state / prop named `null`
today. Adding `null` / `nil` as literals therefore does not clash with
any valid identifier.

## Regression test

`crates/flux-parity/tests/parity.rs::lowercase_null_is_a_known_gap` —
`#[ignore]`d, asserts the current "unbound name" error. When either
fix above lands, un-ignore and assert successful compilation (option 1)
or the hint-bearing error (option 2).

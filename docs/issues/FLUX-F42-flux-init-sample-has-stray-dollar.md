# FLUX-F42 — `flux init` sample writes `${count}` (Kotlin syntax) instead of `{count}`

**Status:** OPEN (fix landed in same commit)
**Found:** round-88 bug hunt (`flux-cli` sweep)
**Severity:** low (a new user scaffolding their first Flux project sees
a sample that renders with a stray `$`: the intended `tapped 5 times`
becomes `tapped $5 times`)

## Summary

`crates/flux-cli/src/init.rs` scaffolds a sample entry file for every
new project:

    Text(text: "tapped ${count} times")

Flux interpolation is **`{expr}`** (spec §ADR-0043); the `${expr}` form
is Kotlin string-template syntax that Flux inherits no meaning for. The
parser reads `$` as an ordinary character and `{count}` as an
interpolation, so the rendered runtime string is `tapped $5 times` — a
literal dollar sign followed by the value.

The `init` scaffold is a user's first impression of Flux, and it ships
with this bug. Every test that exercises the compiler's *own* fixtures
(`handler_body_hot_reload.rs`, `probe_delta_shape.rs`) uses the correct
`{count}` spelling; only the scaffold uses `$` by mistake.

## Reproduction

    $ flux init myapp
    $ cd myapp
    $ flux dev
    # open the app in a host, tap Increment
    # observe "tapped $1 times", not "tapped 1 times"

## Fix

Change the scaffold's string literal to `tapped {count} times`. This
aligns it with the parser's actual interpolation syntax and with the
fixtures the compiler's own test suite uses.

## Regression test

`crates/flux-cli/tests/init_scaffold_compiles.rs` (added in this
commit):
* asserts the sample entry parses;
* asserts the sample entry does not contain a literal `${` (so a
  re-introduction of the Kotlin-style syntax is caught here rather than
  by a user's first run);
* compiles the sample through the full pipeline.

# FLUX-F24 — release-side string escapes are not canonicalized to dev form

**Status:** OPEN (fix landed in same commit)
**Found:** round-86 bug hunt
**Severity:** medium (a Flux program whose preserved string value — a
condition, a `ForEach` collection, a screen route — contains a `$` or a
backslash `\` diverges between dev and the two release paths in
`check_parity`)

## Summary

The dev-path reducer (`crates/flux-parity/src/reduce.rs`) renders strings
from the parsed AST, whose text the Flux parser has **already unescaped**:
a source `"a\\b"` becomes the AST text `a\b`, and `render_string` wraps it
back in quotes. For `$`, the parser has no special rule (Flux uses `{…}`
interpolation, not `$…`), so `"a$b"` has AST text `a$b`.

The release backends re-escape when they emit native source
(`crates/flux-codegen-swift::escape_text` escapes `\` and `"`;
`flux-codegen-kotlin::escape_text` additionally escapes `$`). The
recognizers read that emitted source and call
`crate::bridge::canonicalize_expr`, which translates `\(…)` / `${…}`
interpolation delimiters to `{…}` but does **not** unescape. So a string
carrying an escaped character survives into the reduced release tree with
the escape still present, and the reduced dev tree does not.

## Reproduction

    compo C
      state name: String = "x"
      if name == "a$b" { Text("y") }

* dev reduced tree: `cond: "(name == \"a$b\")"`
* swift reduced tree: `cond: "( name == \"a$b\" )"`
* kotlin reduced tree: `cond: "(name==\"a\\$b\")"` — Kotlin's escape_text
  escaped the `$`.

`check_parity` reports `is_equivalent() == false`. Same divergence for a
backslash: `if name == "a\\b"` gives dev `"a\b"` vs release `"a\\b"`.

## Impact

Conditions are the main surface affected (they are preserved in the reduced
tree, unlike `Text`'s primary positional argument which is dropped by the
parity prop subset). Also screen routes and `ForEach` collections. Any Flux
program whose preserved string value contains `$` or `\` fails parity.

## What the fix does

`canonicalize_release_expr` (new, `crates/flux-parity/src/bridge.rs`) is
used by the two recognizers in place of `canonicalize_expr`. For a
string-literal token it:
* unescapes `\n`/`\t`/`\r`/`\\`/`\"`/`\$` back to their logical chars;
* rewrites `\(expr)` (Swift) and `${expr}` (Kotlin) interpolations to the
  dev-side `{expr}` form (same rule as `canonicalize_expr`, but now
  backslash-aware so an escaped backslash followed by `(` is not
  misread as a Swift interpolation opener);
* leaves any other `\X` as `\X`.

Non-string tokens delegate to `canonicalize_expr` (key paths, numeric
literals, the `unsupported` placeholder collapse, …).

## Related

* `crates/flux-parity/src/bridge.rs` — the fix and its unit tests.
* `crates/flux-parity/src/recognize_swift/swift_views.rs`
* `crates/flux-parity/src/recognize_kotlin/kotlin_views.rs`

# FLUX-F25 — parity recognizers cannot recover a `match` node in the view tree

**Status:** OPEN (known gap; documented by an `#[ignore]`d regression test)
**Found:** round-86 bug hunt
**Severity:** medium (a Flux program using `match` to select a view — a
first-class spec form — diverges across dev/Swift/Kotlin in `check_parity`)

## Summary

The dev reducer (`crates/flux-parity/src/reduce.rs::node_from_ast`) lowers a
`match` in a component body to a `ViewNode::Match` with per-arm `(pattern_label,
children)`. Both release recognizers
(`crates/flux-parity/src/recognize_swift/swift_views.rs`,
`crates/flux-parity/src/recognize_kotlin/kotlin_views.rs`) have **no**
`switch`/`when` dispatch in `parse_body`: they walk into the emitted match
body and recover each arm's view children as siblings at the enclosing level,
losing the `Match` node entirely.

The Kotlin side additionally misinterprets Swift's `where` keyword when
recognizing Swift: `where (s > 90)` has `where` immediately followed by
`(`, which `parse_view` dispatches as a view call — so a Swift `switch` guard
arm contributes a spurious `Primitive { name: "where" }` node.

## Reproduction

    compo C
      state score: Int = 0
      match score {
        s if s > 90 => Text("A")
        s if s > 80 => Text("B")
        _ => Text("F")
      }

* dev reduced tree: `Match { scrutinee: "score", arms: [("s if (s > 90)", [Text]), ("s if (s > 80)", [Text]), ("_", [Text])] }`
* swift reduced tree: `[Primitive { name: "where" }, Text, Primitive { name: "where" }, Text, Text]`
* kotlin reduced tree: `[Text, Text, Text]`

`check_parity` reports `is_equivalent() == false`.

## Why this is filed, not fixed in this round

Fixing it requires:

1. Adding a `switch` dispatcher in `swift_views.rs::parse_body` that recovers
   the arms (`case let s where …:`, `case .Variant(…):`, `case literal:`),
   builds a `ViewNode::Match`, and applies the same guard-condition label form
   the dev reducer uses.
2. Adding a `when (subject) { … }` and `run { val __flux_scrut = … when { … } }`
   dispatcher in `kotlin_views.rs::parse_body` for the two shapes the Kotlin
   emitter now produces (`emit_match` and `emit_match_with_guards`, F20).
3. Deciding whether Swift's `where` (a language keyword) should be added to a
   non-view filter list in the recognizer.

Each step is its own bug surface — the fix is a targeted round of its own, not
a follow-on to the F22–F24 edits.

## Impact

The existing parity snapshots do not exercise this case (`b33_adt` puts the
`match` inside a helper `fn`, not the view body), which is why the gap has
gone unnoticed. Any real Flux app that selects a view via `match` fails parity.

## Regression test

`crates/flux-parity/tests/parity.rs::match_node_in_view_body_is_a_known_divergence`
is `#[ignore]`d and documents the current (divergent) behavior. When the
recognizers gain `switch`/`when` support, the test should be un-ignored and
the assertion flipped.

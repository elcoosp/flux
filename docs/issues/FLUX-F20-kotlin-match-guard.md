# FLUX-F20 — Kotlin codegen cannot express a match guard arm

**Status:** CLOSED (round 58, 2026-10-10) — Kotlin now emits a `when { … }` guard chain
**Found:** round-35 bug hunt
**Severity:** low-medium (the emitted Kotlin is obviously invalid; the parser
fix in round 32 made this code path reachable, but a full lowering does not
exist yet)

## Summary

A Flux guard arm (`s if s > 90 => …`, spec §18.5) is accepted by the parser
as `MatchPatternKind::Guard`, and the UI-tree lowering
(`crates/flux-ir/src/lower/mod.rs`) lowers it to a `NodeKind::Match` whose
children are the arm bodies. It does not carry the guard condition.

`Kotlin::emit_match` (which the codegen calls for every `NodeKind::Match`)
used to emit `is <name> ->` for a guard arm — invalid Kotlin (the RHS of `is`
must be a type) which also silently dropped the guard condition. Round 35
replaced that with a clearly-marked unsupported placeholder
(`/* unsupported match guard: <name> if <cond> */ ->`), which is still invalid
Kotlin but is no longer silently wrong-looking.

`Swift::emit_match` was fixed in the same round because Swift has real
guard syntax: it now emits `case let <name> where <cond>:`.

## What a fix needs

* Either extend the lowering pass to carry the guard condition into the
  `Match` node's metadata (so the emitter can render it), or give the
  emitter access to the surface AST (it already reaches a `Bridge` — check
  whether the guard `cond` is reachable through it).
* For Kotlin: no `when(subject)` guard clause exists in the language today
  (Kotlin 2.0's guarded-when is experimental and syntax is different). The
  faithful lowering is likely an `if/else if` chain that tests the guard
  condition directly, bypassing `when` entirely.
* For Swift: the fix landed in round 35 uses `where` — keep it.
* Add a codegen test that round-trips a guard arm through both backends and
  asserts the condition appears in the emitted source.

## Related

* `docs/issues/FLUX-F19-async-handler-lowering.md` — analogous "codegen can
  render, but the lowering pass rejects" gap for `await` in handlers.

---

## Resolution (round 58, 2026-10-10)

`Kotlin::emit_match` now detects guard arms and routes the whole match
through a new `emit_match_with_guards` helper that emits a Kotlin
`when { … }` Boolean-test chain instead of `when (subject)`:

    run {
        val __flux_scrut = <subject>
        when {
            __flux_scrut.let { s -> (s > 90) } -> { <body> }
            __flux_scrut.let { s -> (s > 80) } -> { <body> }
            else -> { <body> }
        }
    }

The subject is hoisted into a single `val` (evaluated exactly once); each
guard's condition is wrapped in `__flux_scrut.let { name -> … }` so the
binding name is scoped. Literal arms become `(__flux_scrut == lit) ->`;
`Wildcard` (or a bare `_` parsed as `Variant { name: "_" }`) becomes
`else ->`. A variant arm mixed with a guard emits a visible `false ->` arm —
not yet modelled, but never silently mismatches.

Regression test: `crates/flux-codegen-kotlin/tests/f20_guard.rs`.

The Swift side was already fixed in round 35 (real `case let <name> where
<cond>:` syntax).

---

## Resolution (round 58, 2026-10-10)

`Kotlin::emit_match` now detects guard arms and routes the whole match
through a new `emit_match_with_guards` helper that emits a Kotlin
`when { … }` Boolean-test chain instead of `when (subject)`:

    run {
        val __flux_scrut = <subject>
        when {
            __flux_scrut.let { s -> (s > 90) } -> { <body> }
            __flux_scrut.let { s -> (s > 80) } -> { <body> }
            else -> { <body> }
        }
    }

The subject is hoisted into a single `val` (evaluated exactly once); each
guard's condition is wrapped in `__flux_scrut.let { name -> … }` so the
binding name is scoped. Literal arms become `(__flux_scrut == lit) ->`;
`Wildcard` (or a bare `_` parsed as `Variant { name: "_" }`) becomes
`else ->`. A variant arm mixed with a guard emits a visible `false ->` arm —
not yet modelled, but never silently mismatches.

Regression test: `crates/flux-codegen-kotlin/tests/f20_guard.rs`.

The Swift side was already fixed in round 35 (real `case let <name> where
<cond>:` syntax).

# ADR-0047: Unified data-driven release codegen (single primitive registry, one emitter)

**Status:** Accepted (implemented in `flux-codegen-core`, `flux-codegen-kotlin`, `flux-codegen-swift`)
**Date:** 2026-08-28
**Supersedes (retires the duplicated prose):** the two near-identical `nodes.rs` `match`
statements that previously lived in `flux-codegen-kotlin` and `flux-codegen-swift`. Those files
are deleted; their behaviour is preserved byte-for-byte by the shared emitter.
**Decision Drivers:** the two release backends shared ~95% of their primitive emission logic
(`Text`, `Button`, `Image`, `Column`, `Row`, `Router`, `Screen`) in two copy-pasted `match`
arms, while the remaining built-in primitives registered in `flux_types::prelude`
(`CupertinoButton`, `MaterialButton`, `TextField`, `Provider`, `When`, `Switch`) fell through to
a bare `other => "{other}()"` catch-all and were silently emitted as broken placeholder calls.
Adding a primitive meant editing two files in lock-step — the exact drift hazard AGENTS.md
§3.5 / the capability-IDL pattern (ADR-*) exist to prevent.

## Context and Problem Statement

Before this ADR, `flux-codegen-kotlin/src/nodes.rs` and `flux-codegen-swift/src/nodes.rs` each
contained a fully duplicated `emit_primitive`/`emit_container`/`emit_leaf` tree. The only real
divergences were syntactic (indentation unit, interpolation syntax `${}` vs `\(`, `Button(onClick:)`
vs `Button(action:)`, `NavHost` vs `NavigationStack`, `painterResource` vs `UIImage(named:)`,
`sealed interface` vs `enum`, `when`/`is` vs `switch`/`case let`). Everything structural — node-ID
bridging, prop collection, child traversal, `if`/`when`/`ForEach`/`match` emission — was identical
source copied twice.

Concurrently, `flux_types::prelude` seeds **fourteen** adapter/primitive component names, but the
codegen only shaped eight of them. The other six (`CupertinoButton`, `MaterialButton`, `TextField`,
`Provider`, `When`, `Switch`) hit the catch-all and emitted `{Name}()` with no arguments and no
children — present in the output, but not actually shaped. The parity test
(`flux-parity::parity`) recorded this as the committed baseline, so it was "green" but dishonest.

### Verified current state (file:line, pre-refactor)

- `flux-codegen-swift/src/nodes.rs:97` and `flux-codegen-kotlin/src/nodes.rs:99` both end their
  primitive `match` with `other => em.line(indent, &format!("{other}()"))` — the silent fallback.
- `flux_types/src/prelude.rs:95-110` registers all fourteen primitives as the single source of
  truth on the type-checker side; codegen had no corresponding single table.
- AGENT.md §3.5 ("Props are the contract", "one declarative implementation per platform per
  component", "no locally synthesized canonical string ids") and the capability-IDL pattern in
  `flux-devserver/src/capability_idl.rs` already establish "one declarative table, two backends
  read it" as the house style for exactly this kind of dual-backend drift.

## Decision

Introduce a shared `flux-codegen-core` crate that owns **all** codegen traversal and a single
declarative **primitive registry**, and reduce the two release backends to thin
`Backend`-trait implementations that supply only the syntax that genuinely differs.

### Components

- `flux-codegen-core::primitives` — `PrimitiveKind` + `PrimitiveSpec` + `PRIMITIVES`, the single
  source of truth for all fourteen prelude primitives. `PrimitiveKind` classifies each primitive
  (`Container`, `Leaf`, `Button`, `Router`, `Screen`, `Other`); `Other` reproduces the pre-refactor
  bare-call behaviour for `CupertinoButton`/`MaterialButton`/`TextField`/`Provider`/`When`/`Switch`
  (richer native shaping is future work once their dev-model semantics land). Every prelude name
  is present; `primitives::registry_covers_every_prelude_primitive` and
  `registry_has_no_unknown_entries` fail the build if `PRIMITIVES` ever drifts from the prelude.
- `flux-codegen-core::backend::Backend` — a trait carrying only the divergent syntax:
  `INDENT_UNIT`, `CHILD_STEP`, `SCREEN_BODY_STEP`, scalar/collection/interp spellings,
  `container_spacing`, `image_expr`, `router_open/close`, `screen_open/close`, `if_open`,
  `for_each_open/close`, `key_extractor`, `button_open`, `text_field`, `emit_component_header/
  body_open/footer`, `emit_placeholder_component`, `emit_state_cell`, `emit_sum_type`, `emit_match`.
  Structural methods take `&mut Emitter<Self>` and `Self: Sized` so they can reuse the shared
  traversal helpers.
- `flux-codegen-core::emitter::Emitter<B: Backend>` — the generic traverser: node-ID bridge lookup,
  prop collection, `emit_primitive` (container/leaf/button/router/screen/other), `emit_if`,
  `emit_for_each`, `emit_router`, `emit_screen`, `emit_match`, `emit_sum_types`, expression
  rendering. Indentation is unified as `" ".repeat(indent * INDENT_UNIT)`, so Kotlin's 4-space and
  Swift's 1-space layouts fall out of one code path. User component calls (`by_name` → `None`) emit
  `Name(args)` so the parity recognizer (which only treats `Name(...)` as a view) recovers them.
- `flux-codegen-{kotlin,swift}` shrink to `lib.rs` + `backend_impl.rs` (`impl Backend`) +
  `codegen.rs` (orchestration). The old `nodes.rs`/`sumtypes.rs`/`program.rs`/`expressions.rs`/
  `model.rs`/`bridge.rs`/`printers.rs`/`error.rs` are deleted.

### Why this shape (not the alternatives)

- **Keep two duplicated `match` trees, fix the six primitives in both:** rejected — it doubles the
  maintenance surface and the exact drift that caused the gap. AGENTS.md forbids duplicated
  structural logic; the capability-IDL pattern is the ratified alternative.
- **Macros to share the `match`:** rejected — the divergence is syntactic (a trait), not a data
  pattern; a trait is clearer, testable, and lets the `Backend` supply functions, not just strings.
- **Extract only the six missing primitives, leave the eight duplicated:** rejected — partial; the
  eight are the bulk of the duplication and the highest drift risk.

## Consequences

### Positive

- Adding a primitive is a one-line edit to `PRIMITIVES` (plus, where it diverges, one trait method
  per backend) — not a touch to two duplicated `match` statements. The parity test makes silent
  omission impossible.
- The two backends are provably in sync: they share one emitter, so a structural change cannot land
  in one and not the other.
- Byte-for-byte output is preserved for every Appendix B.3 example: all 30 codegen pipeline
  integration snapshots (16 Kotlin + 14 Swift) pass, and `flux-codegen-swift`'s
  `generated_swift_parses` test — which shells out to `swiftc -parse` on the combined generated
  output — accepts the emitted SwiftUI (the output genuinely compiles).
- `flux-parity` is green for all ten B.3 examples (dev == swift == kotlin structurally).

### Negative / trade-offs

- `flux-codegen-core` is a new workspace member and a new dependency edge for both backends.
- The six previously-bare primitives now have real native shaping: `CupertinoButton` /
  `MaterialButton` lower to styled native `Button`s (SwiftUI `.buttonStyle(.bordered)` /
  `.buttonStyle(.borderedProminent)`; Compose `RoundedCornerShape(12.dp)`), `TextField`
  lowers to a native editable field bound to `text`/`onChange` with an optional
  `placeholder`, and `Provider` lowers to a child-bearing wrapper. `When` / `Switch`
  are control-flow forms: they lower to `NodeKind::If` / `NodeKind::Match` and are
  emitted structurally by `emit_if` / `emit_match`, so they never reach `emit_primitive`
  as a primitive call. `CupertinoButton` / `MaterialButton` normalize to `Button` in the
  parity contract so the release output compares equal to the dev surface AST.
- One parity snapshot (`parity_b38_platform`) was regenerated: the unified `render_expr` spells an
  unsupported `platform() == "ios"` condition as `( 0 /* unsupported */ == "ios" )` rather than the
  old `( /* unsupported expr */ 0 == "ios" )`. The structural parity contract is unchanged; only the
  recorded comment text shifted.

### Neutral

- `Backend` trait methods that need shared state take `&mut Emitter<Self>` and require `Self: Sized`;
  this is the standard Rust pattern for trait methods that borrow `self`'s generic parameter.

## Implementation status

- `flux-codegen-core`, `flux-codegen-kotlin`, `flux-codegen-swift` rewritten and committed.
- `cargo clippy -p flux-codegen-core -p flux-codegen-kotlin -p flux-codegen-swift --all-targets
  -- -D warnings` is clean; `cargo fmt` is clean.
- `cargo test -p flux-codegen-core -p flux-codegen-kotlin -p flux-codegen-swift -p flux-parity`
  is green (including the `swiftc -parse` acceptance check).
- The `flux-codegen-core::parity` module asserts `PRIMITIVES` ↔ `flux_types::prelude` coverage.

## References

- AGENTS.md §3.5 (adapter contract, one declarative implementation per platform per component).
- `flux-devserver/src/capability_idl.rs` — the ratified "one declarative table, two backends read
  it" pattern this ADR mirrors for primitives.
- `flux_types/src/prelude.rs` — the type-checker-side single source of truth for primitive names.
- Appendix B.3 (the ten examples) and `flux-parity` (the parity acceptance harness).

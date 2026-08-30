---
id: FLUX-056
status: partial
blocked_by:
  - PRD-M
lane: LANE-H
phase: "Phase 5"
labels:
  - perf
  - benchmark
source: CHANGELOG.md §PRD-T (deferred: "the large-list scroll benchmark requires PRD-N's ScrollView (blocked_by: [PRD-J, PRD-M, PRD-N])")
related_adrs:
  - ADR-0047
---

# FLUX-056: Large-list scroll benchmark (depends on ScrollView + perf harness)

> **Status (2026-08-30):** the `ScrollView` primitive (PRD-N) leg is **DONE**;
> the 1k/10k-item *virtualized* scroll **benchmark** leg remains `blocked`.
> - `flux-perf-harness` (PRD-J / FLUX-066) now exists as a crate and is wired
>   into CI — that dependency is resolved.
> - `ScrollView` (PRD-N) now exists: registered in the ADR-0047 codegen
>   registry, seeded in the `flux-types` prelude, declared in stdlib
>   (`stdlib/scrollview.flux`), wired into both dev-host adapter kits
>   (`ScrollViewAdapter` on Android + iOS), with a `flux-parity` dev/release
>   trace test (`flux_056_scrollview_pins_dev_release_mapping`).
> - The remaining blocker is the *virtualized* 1k/10k scroll benchmark. Per the
>   original issue, MLP has **no virtualization** (`ForEach` is explicitly
>   non-virtualized), so a virtualized-scroll benchmark would measure
>   diff/reconcile latency, not scroll — authoring it is deferred. The benchmark
>   leg is therefore still `blocked` (only by PRD-M CI hardening now).

- **Lane:** LANE-H (Phase 5)
- **Dependencies resolved:** PRD-N (`ScrollView`), PRD-J (perf harness crate)
- **Depends on (remaining):** PRD-M (CI hardening) for the benchmark leg
- **Source:** `CHANGELOG.md` §PRD-T deferred
- **Related ADRs:** ADR-0047

## Closure — `ScrollView` primitive leg (DONE)

Delivered end-to-end, mirroring the FLUX-037 PRD-N `ScrollView`-template:

- **Codegen registry:** `ScrollView` added to `PRIMITIVES` (`Container` kind,
  `kotlin_view: "ScrollView"`, `swift_view: "ScrollView"`) in
  `flux-codegen-core/src/primitives.rs`; `view_tree::is_container` extended so
  the parity reducer walks its children; `HostAdapterSpec::ScrollView` added to
  `HOST_ADAPTERS` (both platforms).
- **Type-checker prelude:** seeded `ScrollView` in `flux-types/src/prelude.rs`;
  the `registry_covers_every_prelude_primitive` / `registry_has_no_unknown_entries`
  guards extended.
- **Stdlib:** `stdlib/scrollview.flux` declares `compo ScrollView(orientation:
  Option[String] = None)`; `required_primitive_declarations_exist` in
  `flux-parity/tests/stdlib_parse.rs` updated.
- **Dev-host adapters (both platforms):**
  - Android: `adapters/ui-kotlin/.../ScrollViewAdapter.kt`
    (`SCROLL_ORIENTATION` prop index), registered in `FluxUiKit.adapters`;
    `ShadowTreeRenderer.RenderScrollView` wraps children in a Compose
    `verticalScroll`/`horizontalScroll` modifier.
  - iOS: `adapters/ui-swift/.../ScrollViewAdapter.swift` (`UIScrollView`),
    registered in `AdapterKit.AdapterRegistry`.
- **Parity test:** `flux-parity::flux_056_scrollview_pins_dev_release_mapping`
  pins the dev/release node mapping (vertical default + explicit horizontal).

> **Verification note:** `flux-codegen-core` + `flux-types` unit/registry tests
> are green and `cargo clippy -D warnings` is clean for those crates. The
> `flux-parity` integration test + the `native_kit_parity` kit-match guard could
> **not** be run: `flux-parity` depends on `flux-ir-serde`, which is currently
> broken by an in-flight telemetry migration (FLUX-060) elsewhere in the
> workspace (non-exhaustive `match` on new `TelemetryEvent` variants). Once that
> migration lands, run `cargo nextest -p flux-parity` to generate the
> `parity_flux_056_scrollview` snapshot and confirm green.

## Problem Statement

PRD-T deferred "the large-list scroll benchmark [which] requires PRD-N's
`ScrollView`" (`blocked_by: [PRD-J, PRD-M, PRD-N]`). The §3.10 large-list scroll
budget is unverified without it.

## Solution

Once `ScrollView` (PRD-N) lands, add a 1k/10k-item virtualized scroll benchmark to
`flux-perf-harness` feeding the §3.10 scroll budget gate, run in CI.

## Implementation Decisions

- Reuses `flux-perf-harness`'s `MetricRecord` schema (PRD-J) so it shares the CI
  gate.
- Measures scroll-frame latency + reconciliation ratio on the virtualized list.

## Testing Decisions

- The bench runs in `perf-harness.yml` and fails CI if the scroll budget regresses.

## Out of Scope

- The RN/Flutter published comparison (FLUX-057).

---
id: FLUX-077
status: done   # iOS kit reaches full parity with Android for the FLUX-077 set: Stack/Grid/Spacer/SafeArea/Modal/Sheet/Dialog/Animate (degraded container form, present since af47415, wired in AdapterKit.swift:377-386) + Toggle (added 2026-08-30, both kits). Verified: `xcodebuild build -scheme FluxApp` compiles FluxUIKit (all 9 adapters); RenderMountTests green incl. testRegistryResolvesTogglePrimitive; `:adapters:ui-kotlin:test` green.
lane: LANE-N
phase: "Phase 2"
blocked_by:
  - FLUX-037
  - FLUX-038
  - FLUX-042
labels:
  - stdlib
  - primitive
  - ios
  - parity
source: FLUX-037 / FLUX-038 / FLUX-042 parity gate (AGENTS.md — a primitive needs BOTH adapter kits before advertising). Layout/overlay/animation primitives landed on Android + stdlib but have NO iOS adapter kit, so they are not advertised.
related_adrs:
  - ADR-0047
---

> **Closure note (2026-08-30):** The issue's premise was partly inaccurate.
> When picked up, the eight `Stack`/`Grid`/`Spacer`/`SafeArea`/`Modal`/`Sheet`/
> `Dialog`/`Animate` adapters **already existed** in `adapters/ui-swift`
> (committed in `af47415`, the todo-render agent's FLUX-037/038/042 pass) and
> were already at parity with Android: both kits degrade those eight to a plain
> container carrying children, gated on the ADR-0048 iOS dev-tier convergence
> decision (AGENTS.md §0.2 — a SwiftUI rewrite is explicitly out of scope, so
> the degraded form is the *correct* unified-tier mapping, not a stub to fix).
> The genuine gap was **`Toggle`**: it was used by `examples/todo` and seeded in
> the Rust prelude + codegen (`swift_view: "Toggle"`), but had **no adapter on
> either platform** — so the todo `TaskRow` degraded to a blank container on iOS.
> This issue therefore delivered `Toggle` to *both* kits (Android was also
> missing it), with identical prop contracts (`value` / `onValueChange` /
> `enabled`), FNV-1a name-derived prop indices mirrored on both sides, per-node
> factories (FLUX-007), and weakly-held executor dispatch. iOS now reaches full
> parity with Android for the FLUX-077 primitive set, and `examples/todo`
> renders its toggle. Tests: `ToggleAdapterTests` (SwiftPM `FluxUIKit` suite) +
> `testRegistryResolvesTogglePrimitive` (runtime `FluxAppTests`, drives the real
> `UISwitch` on the simulator); Android `FormGestureAdapterTest` got `toggle`
> cases + a registry-resolution assertion.
>
> Verification: `./gradlew :adapters:ui-kotlin:test` green (incl. new Toggle
> cases); `xcodebuild test -scheme FluxApp` green for `RenderMountTests` (incl.
> the new registry test). Two pre-existing, unrelated breaks were left
> untouched: `:runtimes:android:host` `FrameDeserializer.kt:464` (compile), and
> `runtimes/ios/Tests/RenderPerfHarnessTests.swift:106` (`let children`); plus a
> `CapabilityRoundTripTests.testHttpGetJsonResolvesToRecordViaResolver` JSON
> fixture failure. Those are out of scope for this issue.

# FLUX-077: iOS adapter parity for FLUX-037 layout + FLUX-038 overlay + FLUX-042 animation primitives

## Problem Statement

FLUX-037 (layout: `Stack`/`Grid`/`Spacer`/`SafeArea`), FLUX-038 (overlay:
`Modal`/`Sheet`/`Dialog`), and FLUX-042 (signal-graph animation primitive) have
landed on **Android + the stdlib** but are **not yet advertised** to authors.
AGENTS.md is explicit: *a primitive needs BOTH adapter kits before advertising.*
iOS (`adapters/ui-swift`) still has **no** `Stack` / `Grid` / `Spacer` /
`SafeArea` / `Modal` / `Sheet` / `Dialog` / `Animate` adapters, so these
primitives cannot be seeded into the Swift prelude / `prelude.flux` public
surface or documented as generally available.

Additionally, `Toggle` (used by `examples/todo`) has no iOS adapter
(`ToggleAdapter.swift` missing) — it is part of the data-driven surface
(FLUX-072) but has no standalone parity issue; it is included here so the iOS
kit reaches full parity with the Android kit in one pass.

The Android side that already landed (the reference contract to mirror):

- `adapters/ui-kotlin`: `StackAdapter`, `GridAdapter`, `SpacerAdapter`,
  `SafeAreaAdapter` (FLUX-037), `ModalAdapter`, `SheetAdapter`, `DialogAdapter`
  (FLUX-038), `AnimateAdapter` (FLUX-042). Each is a declarative adapter that
  reads props by name via `PropsIndex.propIndexForName` (FNV-1a-32 name digest,
  never a hardcoded position — AGENTS.md §3.2) and records resolved view
  properties onto a `FluxNativeView`.
- `PropsIndex.kt`: each prop index is derived from the **FNV-1a-32 name digest**
  (`propIndexForName`), never a hardcoded position. The Swift kit must derive
  the same indices identically.
- `stdlib/`: `stack.flux`, `grid.flux`, `spacer.flux`, `safearea.flux`,
  `modal.flux`, `sheet.flux`, `dialog.flux`, `animate.flux` — `compo`
  declarations with the layout/overlay/animation signal contract.
- Per-node adapter factories (no shared singletons — FLUX-007), weakly-held
  executor, handler dispatch through `FluxExecutor.dispatch`.

> **Coordination note:** the `examples/todo` render agent is also working in
> `adapters/ui-swift` and may land `Stack` / `Spacer` / `Toggle` first (todo uses
> those three). Do **not** duplicate those files if they already exist when this
> issue is picked up — verify with `ls adapters/ui-swift/Sources/FluxUIKit/` and
> only add the adapters still missing. This issue owns the *complete* iOS parity
> set: Stack, Grid, Spacer, SafeArea, Modal, Sheet, Dialog, Animate, Toggle.

## Solution

Port the nine adapters to `adapters/ui-swift` following the existing Swift dev
adapter contract (the `FluxUIKit`/`FluxAdapter` equivalents). Map each to its
native control:

- `Stack` → `ZStack` (z-order overlay), `Grid` → `LazyVGrid`/`Grid`,
  `Spacer` → `Spacer` (with `weight:` → `frame(minWidth/minHeight:)` grow),
  `SafeArea` → `safeAreaInset` / `.safeAreaPadding`.
- `Modal` / `Sheet` / `Dialog` → SwiftUI `sheet`/`fullScreenCover`/`alert`
  driven by a bound presentation signal, reconciling children by stable node id.
- `Animate` → a SwiftUI `.animation`/`.withAnimation` wrapper keyed to the signal
  the Android `AnimateAdapter` observes (signal-graph driven, not discrete
  patches).
- `Toggle` → SwiftUI `Toggle` with `value:`/`onValueChange:` bound to the same
  signal the Android `Toggle`-equivalent uses.

Resolve prop indices through the same FNV-1a name digest so the iOS host stays
in lockstep with the dev server. Add the Swift-side XCTest equivalents of the
Android `LayoutOverlayAdapterTest` (prop mapping, keyed reconciliation, handler
binding).

## Implementation Decisions

- Mirror the Android prop-index set exactly (same names → same derived indices).
- Do **not** advertise the primitives (seed `prelude.flux` / public surface)
  until this issue is `done`; the FLUX-037/038/042 docs carry the parity gate.
- Reuse any `Stack`/`Spacer`/`Toggle` adapters the todo-render agent lands first
  (see Coordination note) rather than overwriting them.

## Testing Decisions

- XCTest asserting each adapter's dev/release mapping and that a presentation
  signal / toggle / animation drives the bound handler — parity with the Android
  `LayoutOverlayAdapterTest` (and the todo app rendering these on a device).

## Out of Scope

- The Android side (already landed), the stdlib `.flux` sources (already landed),
  and the form/gesture primitives (those are FLUX-076, a separate iOS-parity
  issue).

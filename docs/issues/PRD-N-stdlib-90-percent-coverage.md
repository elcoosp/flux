---
id: PRD-N
status: open
lane: LANE-N
phase: "Phase 2"
blocked_by:
  - PRD-J
labels:
  - epic
  - prd
  - stdlib
  - ui
  - ios
  - android
  - coverage
source: docs/roadmaps/flux-roadmap-to-1.0.md §1.1,§2
related_adrs:
  - ADR-0047
  - ADR-0027
---

# PRD-N: Stdlib Expansion to 90% Use-Case Coverage

- **Lane:** LANE-N (Phase 2)
- **Depends on:** PRD-J (rendering model must be settled before new primitives are built against one tier)
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §1.1, §2
- **Related ADRs:** ADR-0047 (unified data-driven codegen primitive registry), ADR-0027
  (node-ID bridge), AGENTS.md §3.5 (adapter contract), §0.2 (unified tier)

## Problem Statement

The stdlib is 8 primitives (`text`, `button`, `column`, `row`, `text_field`, `screen`, `router`,
plus `color`/`font`/`platform`/`traits`/`capabilities` declarations). There is no `Image`, no
scrollable list, no `Modal`/`Sheet`, no gestures, no animation primitive, no `Stack`/`Grid` beyond
row/column, no forms beyond a single text field. This is nowhere near the 1.0 bar of "a developer
can build a typical CRUD / social / e-commerce app without dropping to native code." This PRD tracks
primitive-by-primitive expansion ordered by frequency of appearance in a typical mobile app.

## Solution

Add stdlib primitives in priority order, each built against *one* rendering model on both platforms
(once PRD-J settles it): layout & scrolling (`ScrollView`/virtualized `List`, `Stack`, `Grid`,
`Spacer`, `SafeArea`, `Modal`/`Sheet`/`Dialog`); media & input (`Image`, `Icon`, form primitives
`Switch`/`Checkbox`/`Slider`/`Picker`/`DatePicker`/`TextArea`, form validation, `Gesture`
long-press/swipe/drag/pinch); motion (signal-graph-tied animation primitive); data & networking
(HTTP capability + structured persistence — see PRD-Q); theming & accessibility (design tokens
codegen'd into SwiftUI/Compose themes, a11y props threaded from day one).

## User Stories

1. As a Fluff app developer, I want a virtualized `List`/`ScrollView`, so that an app with more than a
   handful of items actually works.
2. As a Fluff app developer, I want `Stack`/`Grid`/`Spacer`/`SafeArea`, so that I can lay out real
   screens without nesting row/column hacks.
3. As a Fluff app developer, I want `Modal`/`Sheet`/`Dialog` with a real transition contract, so that
   I can present overlays natively.
4. As a Fluff app developer, I want `Image` (local + remote with caching) and themed `Icon`, so that
   media renders without a WebView escape hatch.
5. As a Fluff app developer, I want form primitives (`Switch`/`Checkbox`/`Slider`/`Picker`/
   `DatePicker`/`TextArea`) + validation composition, so that I can build forms beyond one text field.
6. As a Fluff app developer, I want gesture primitives (long-press/swipe/drag/pinch) in addition to the
   existing tap, so that my app feels native.
7. As a Fluff app developer, I want an animation primitive tied into the signal graph, so that motion
   drives signals (not just discrete patches) and matches SwiftUI/Compose native animation.
8. As a Fluff app developer, I want design tokens (spacing/color/typography) codegen'd into both
   platform theme mechanisms, so that theming is not hardcoded literals per component.
9. As a Fluff app developer, I want accessibility props (labels/roles/focus order) on every new
   primitive from day one, so that I do not have to retrofit a11y after 40 components ship.
10. As a Flux core engineer, I want each new primitive to extend the adapter contract on both
    `adapters/ui-kotlin` and `adapters/ui-swift`, register in the dev VM stdlib, and map in
    `flux-codegen-core`'s primitive registry (ADR-0047), so that dev and release stay in lockstep.

## Implementation Decisions

- **One rendering model, both platforms:** because PRD-J settles the iOS tier, every new primitive is
  built once against the unified tier contract (AGENTS.md §0.2 / §3.5), not forked into a second iOS
  mapping.
- **Adapter contract is the interface:** new primitives are added through `FluxNativeView.setProperty`
  intent on both kits; hosts translate to real views. Per-node adapter factories (never shared
  singletons, FLUX-007) and keyed reconciliation (never recreate existing nodes, AGENTS.md §3.5) apply
  as for existing primitives.
- **Prop indices derived, never hardcoded** (AGENTS.md §3.2): every new prop uses
  `prop_index_for_name` / `PropsIndex.propIndexForName`; no positional literals.
- **Codegen registry:** the ADR-0047 primitive registry is the single source of the dev↔release mapping;
  a new primitive is "done" only when both the dev VM registry and the codegen template carry it.
- **a11y from day one:** accessibility props are part of the initial adapter contract for each primitive,
  not a follow-up PRD — retrofitting is explicitly called out as more expensive in the roadmap.
- **Animation touches the signal graph:** the motion primitive drives signals (spring/timing curves),
  reusing the existing reactive core rather than a separate animation subsystem.

## Testing Decisions

- **Good test:** for each primitive, a dev/release parity test (via `flux-parity`) asserting the same
  `.flux` source materializes equivalent props on both backends; plus an adapter-contract test asserting
  missing/renamed props degrade to default (never throw, AGENTS.md §3.5). Not tests of kit-internal view
  code.
- **Modules to test:** the dev VM stdlib registry entries, the `flux-codegen-core` primitive-registry
  mappings, and the per-platform adapter property setters (JVM-host + iOS host, both Android-free /
  simulator-testable where possible).
- **Prior art:** existing `text`/`button`/`column`/`row` adapter + codegen mappings are the template to
  clone per primitive. `flux-parity`'s dev/release trace diff is the parity oracle.

## Out of Scope

- The HTTP / structured-persistence / WebView capabilities themselves (PRD-Q) — this PRD covers the UI
  primitives; the data behind `Image` remote caching and forms persistence lands with capabilities.
- The iOS/Android render-tier decision (PRD-J) — a prerequisite, not in scope here.
- DevTools visualization of new primitives (PRD-P).
- LSP autocomplete for new props (PRD-O).

## Further Notes

This is the largest PRD by surface area and is intended to be sliced into one sub-PRD/issue per
primitive family when issues are created. It must not start until PRD-J closes the rendering model.

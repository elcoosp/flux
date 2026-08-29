---
title: Migrating from Flutter
description: An honest map from Flutter widgets to Flux primitives — and what Flux does not yet cover.
---

If you come from Flutter, Flux's declarative component model will feel familiar:
a tree of widgets/components, rebuilt from state. This guide maps Flutter widgets
to Flux primitives, links each to the real component, and names the gaps plainly.

## Widget → component map

| Flutter | Flux | Notes |
|---|---|---|
| `Column(children: [...])` | [`Column`](/guides/cookbook/column/) | `gap` replaces `SizedBox` spacers between children |
| `Row(children: [...])` | [`Row`](/guides/cookbook/row/) | same |
| `Text('...')` | [`Text`](/guides/cookbook/text/) | interpolation via `{expr}` |
| `ElevatedButton` / `TextButton` | [`Button`](/guides/cookbook/button/) | `onPress` is a `Handler` closure |
| `TextField` | [`TextInput`](/guides/cookbook/textinput/) | controlled — write the handler payload back to a signal |
| `Image.asset(...)` | [`Image`](/guides/cookbook/image/) | `source` is an asset path |
| `Navigator` / `MaterialPageRoute` | [`Router`](/guides/cookbook/router/) + [`Screen`](/guides/cookbook/screen/) | `Router.navigate(route)` swaps screens |
| `Scaffold` | a `Column`/`Row` root (no direct equivalent) | compose your own layout |
| `Container` / `Padding` | per-component props + `gap` | no standalone `Padding` widget |

## State

| Flutter | Flux |
|---|---|
| `StatefulWidget` + `setState` | `compo` with `state` signals; assign in a handler |
| `ValueNotifier` / `ChangeNotifier` | a signal; derived signals replace `Selector`/`ValueListenableBuilder` — see [State management](/guides/state-management/) |
| `InheritedWidget` / `Provider` | global stores over the signal graph (see [State management](/guides/state-management/)) |
| `FutureBuilder` | a capability call returning a `Result` cell (ADR-0045) |
| `StreamBuilder` | not yet; async data through capabilities only for now |

## What Flux does NOT yet cover (be honest)

- **Dynamically-sized lists.** No `ListView.builder` equivalent in the MLP
  pipeline — `ForEach` lowers to an empty splice and handler writes target
  fixed signal names. Model fixed slots (see the [Todo
  example](https://github.com/elcoosp/flux/tree/main/examples/todo)). Real
  growable lists are on the roadmap.
- **The full `Widget` catalog.** Only stdlib primitives exist; no `Card`,
  `Chip`, `Slider`, etc. yet (theming/design tokens are FLUX-043).
- **Implicit animations / `AnimatedBuilder`.** Animation primitives are a
  roadmap item (FLUX-042) — today you drive values through signals and
  capabilities.
- **Web / desktop targets.** Native iOS/Android only.

For ~90% of app screens (forms, navigation, state, capability-backed data) Flux
covers the ground. The gaps above are where you'd reach for a native escape
hatch (FLUX-046/048) or wait for the roadmap item.

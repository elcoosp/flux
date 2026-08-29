---
title: Migrating from React Native
description: An honest map from React Native concepts to Flux primitives — and what Flux does not yet cover.
---

Flux is a write-once UI language that compiles to native iOS/Android. If you know
React Native, most of the vocabulary transfers. This guide maps RN concepts to
Flux, links each to the real primitive, and names the gaps plainly (Flux targets
~90% of real use cases today — not 100%).

## Component model

| React Native | Flux | Notes |
|---|---|---|
| `function Component()` returning JSX | `compo Name` with an indented body | Flux components are declared with `compo`, children are indented beneath the view call |
| `<View style={{flexDirection:'column'}}>` | [`Column`](/guides/cookbook/column/) | Flux splits RN's `View` into `Column`/`Row` by design |
| `<View style={{flexDirection:'row'}}>` | [`Row`](/guides/cookbook/row/) | see above |
| `<Text>` | [`Text`](/guides/cookbook/text/) | string interpolation via `{expr}` |
| `<Button>` | [`Button`](/guides/cookbook/button/) | `onPress` is a `Handler` closure |
| `<TextInput>` | [`TextInput`](/guides/cookbook/textinput/) | public name mirrors RN (ADR-0038) |
| `<Image>` | [`Image`](/guides/cookbook/image/) | `source` is an asset path |
| `<Navigator>` / `react-navigation` | [`Router`](/guides/cookbook/router/) + [`Screen`](/guides/cookbook/screen/) | navigate via `Router.navigate(route)` capability |

## State

| React Native | Flux |
|---|---|
| `useState` | `state name: Type = initial` at the top of a `compo` |
| `setState(v)` | assign the signal: `name = v` inside a handler |
| derived value in render | a derived signal — see [State management](/guides/state-management/) |
| context / redux | global stores over the signal graph (see [State management](/guides/state-management/)) |

## Effects & lifecycle

| React Native | Flux |
|---|---|
| `useEffect(...)` | `onMount` / `onCleanup` lifecycle hooks (run through the VM) |
| async data fetch | a capability call (e.g. `Storage`, `Http`) returning a `Result` cell — see ADR-0045 |

## What Flux does NOT yet cover (be honest)

These are real gaps as of this writing — call them out rather than pretend:

- **Dynamically-sized lists.** The MLP lower pass emits a `ForEach` as an empty
  splice, and handler `WRITE_SIGNAL` targets are statically-fixed signal names
  with no list set/remove/slice ops. A growable `FlatList` equivalent is **not**
  expressible in the current dev pipeline — model fixed slots instead (see the
  [Todo example](https://github.com/elcoosp/flux/tree/main/examples/todo), which
  uses five fixed task slots). Tracked by the list-comprehension and large-list
  roadmap items.
- **Stylesheets / flex props.** There is no `StyleSheet.create` or arbitrary
  `style` prop. Layout is expressed through `Column`/`Row` `gap`/`alignment` and
  per-component props. Design tokens / theming are on the roadmap
  (FLUX-043).
- **Third-party component libraries.** Only the stdlib primitives exist. Native
  interop goes through the WebView / native-module escape hatches
  (FLUX-046/048).
- **Web target.** Flux renders natively on iOS/Android only. There is no web
  backend today.

If your app depends heavily on one of these, Flux may not be the right fit yet —
but the common 90% (forms, navigation, state, capabilities) is covered.

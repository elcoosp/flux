---
title: Screen
description: A single routable screen addressed by its route string.
---

Contract (from `stdlib/router.flux`):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `route` | `String` | — | required; the route key the `Router` switches on |

`Screen` wraps a single content child and is addressed by its `route` string.
The host reads this prop via `FNV-1a("route")` to pick the visible screen, so
**the prop name `route` is load-bearing and must never be renamed**
([AGENTS.md §memory](https://github.com/elcoosp/flux/blob/main/AGENTS.md)).

```flux
Router initialRouteName: "home"
  Screen route: "home"
    Text text: "Home"
  Screen route: "settings"
    Text text: "Settings"
```

A `Screen` typically contains a [`Column`](/guides/cookbook/column/) with the
screen's content. See the [Router cookbook
page](/guides/cookbook/router/) for imperative navigation, and the [Todo
example](https://github.com/elcoosp/flux/tree/main/examples/todo) for a
two-screen app. Native rendering: a pushed view controller / fragment in dev;
a SwiftUI `NavigationStack` destination / Compose `NavHost` composable in
release (Appendix F.7).

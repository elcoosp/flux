---
title: Router
description: Stack navigation driven by Screen children and their route strings.
---

Contract (from `stdlib/router.flux`):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `initialRouteName` | `String` | `"home"` | the first screen shown |

`Router` owns a stack of [`Screen`](/guides/cookbook/screen/) children and drives
platform navigation. Each `Screen` is addressed by its `route` prop — the host
reads `route` via `FNV-1a("route")` to pick the visible screen (ADR-0045), so
**the prop name `route` must never be renamed**.

Navigate imperatively through the `Router` capability (cap 3 / method 1):

```flux
compo App
  Router initialRouteName: "tasks"
    Screen route: "tasks"
      Column
        Text text: "Tasks"
        Button text: "About", onPress: || { Router.navigate("about") }
    Screen route: "about"
      Column
        Text text: "About"
        Button text: "Back", onPress: || { Router.navigate("tasks") }
```

`Router.navigate("about")` swaps the visible screen by writing the route signal
(signal 97). Screen state is preserved across push/pop by the keyed reconciler
([AGENTS.md §3.5](https://github.com/elcoosp/flux/blob/main/AGENTS.md)). See the
[Todo example](https://github.com/elcoosp/flux/tree/main/examples/todo) for a
working two-screen app. Native rendering: `UINavigationController` /
`FrameLayout` stack in dev; SwiftUI `NavigationStack(path:)` / Compose `NavHost`
in release (Appendix F.6).

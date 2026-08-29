---
title: Button
description: A tappable action that fires an onPress handler.
---

Contract (from `stdlib/button.flux`):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `text` | `String` | — | required; the label |
| `onPress` | `Handler` | — | required; fired on tap |
| `enabled` | `Bool` | `true` | when `false`, ignores taps |
| `color` | `Option[Color]` | `None` | background tint |

Minimal:

```flux
compo Counter
  state count: Int = 0
  Column
    Text text: "tapped {count} times"
    Button text: "Increment", onPress: || { count = count + 1 }
```

The `onPress` is a `Handler` — a closure with no arguments. This is the verb
Flux mirrors from React Native's `Button`. See the [Counter
example](https://github.com/elcoosp/flux/tree/main/examples/counter) for the
canonical usage. Native rendering: `UIButton` / `android.widget.Button` in dev;
SwiftUI `Button` / Compose `Button` in release (Appendix F.2).

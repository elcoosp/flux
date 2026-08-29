---
title: Text
description: Render a string with optional font, size, color, alignment, and overflow.
---

Contract (from `stdlib/text.flux`):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `text` | `String` | — | required; supports `{expr}` interpolation |
| `font` | `Option[Font]` | `None` | see `stdlib/font.flux` |
| `size` | `Option[Float]` | `None` | point size override |
| `color` | `Option[Color]` | `None` | see `stdlib/color.flux` |
| `alignment` | `Option[Alignment]` | `None` | cross-axis text alignment |
| `maxLines` | `Option[Int]` | `None` | truncate beyond N lines |
| `overflow` | `Option[Overflow]` | `None` | how to truncate |

Minimal:

```flux
Text text: "Hello, Flux"
```

Interpolated from a signal:

```flux
compo Greeting
  state name: String = "world"
  Column
    Text text: "Hello, {name}"
```

Used in the [Todo example](https://github.com/elcoosp/flux/tree/main/examples/todo)
for every label. Native rendering: `UILabel` / `TextView` in dev; SwiftUI
`Text` / Compose `Text` in release (Appendix F.1).

---
title: Column
description: A vertical stack with configurable gap and cross-axis alignment.
---

Contract (from `stdlib/column.flux`):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `gap` | `Float` | `0.0` | spacing between children |
| `alignment` | `Option[Alignment]` | `None` | cross-axis alignment |

Minimal — children are supplied as an indented block under the view call:

```flux
Column gap: 16.0
  Text text: "Title"
  Button text: "Go", onPress: || { }
```

`Column` is a container: list its children indented beneath it. Native rendering:
`UIStackView(axis: .vertical)` / `LinearLayout(VERTICAL)` in dev; SwiftUI
`VStack(spacing:)` / Compose `Column(spacing:)` in release (Appendix F.3).

> Note: `Column` and `Row` are Flux's own names (divergent from React Native's
> `View` + `flexDirection` by design — see the [migration
> guide](/guides/migrate-from-rn/)).

---
title: Row
description: A horizontal stack with configurable gap and cross-axis alignment.
---

Contract (from `stdlib/row.flux`):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `gap` | `Float` | `0.0` | spacing between children |
| `alignment` | `Option[Alignment]` | `None` | cross-axis alignment |

Minimal:

```flux
Row gap: 8.0
  Text text: "Label"
  Button text: "Action", onPress: || { }
```

`Row` is the horizontal sibling of [`Column`](/guides/cookbook/column/). Native
rendering: `UIStackView(axis: .horizontal)` / `LinearLayout(HORIZONTAL)` in dev;
SwiftUI `HStack(spacing:)` / Compose `Row(spacing:)` in release (Appendix F.4).

> `Row`/`Column` are Flux's own names (divergent from React Native's `View` by
> design — see the [migration guide](/guides/migrate-from-rn/)).

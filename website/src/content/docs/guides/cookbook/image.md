---
title: Image
description: Render a bitmap from an asset path with explicit size and resize mode.
---

Contract (from `stdlib/image.flux`; mirrors the React Native `Image` surface):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `source` | `String` | — | required; asset path relative to project root, e.g. `"assets/logo.png"` |
| `width` | `Option[Float]` | `None` | explicit width |
| `height` | `Option[Float]` | `None` | explicit height |
| `resizeMode` | `Option[String]` | `None` | `"fill"` (default) \| `"fit"` \| `"stretch"` |

Minimal:

```flux
Image source: "assets/flux.png", width: 96.0, height: 96.0
```

In dev the bitmap is fetched over HTTP from the dev server's asset route, so the
path is relative to the project root you ran `flux dev` from. In release the
asset is bundled. See the **About** screen in the [Todo
example](https://github.com/elcoosp/flux/tree/main/examples/todo) for a real
usage. Native rendering: `UIImageView` / `ImageView` in dev; SwiftUI `Image` /
Compose `Image` in release (Appendix F.8).

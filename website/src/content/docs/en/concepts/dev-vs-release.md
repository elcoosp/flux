---
title: Dev vs Release
description: How Flux runs in dev (interpreted, patched) versus release (codegen'd native SwiftUI / Compose).
---

import { Card, CardGrid } from '@astrojs/starlight/components';

Flux has two execution modes. Both consume the **same Reactive Tree IR** produced
by the dev server's lowering pass — they differ only in how the host turns that
IR into pixels.

## Dev mode — interpreted and patched

In dev, a precompiled **host app** ships to the device. The dev server parses
`.flux`, lowers it to the Reactive Tree IR, diffs it against the previous tree,
and ships **binary patches** (Appendix D) over a WebSocket. The host's embedded
register-based bytecode VM and SolidJS-style signal graph apply the patch and
mutate a shadow tree of native views.

- Fast iteration: save → diff → patch, no recompile of the app.
- Full introspection: the wire frame, the VM outcome, and the reconcile trace
  are all observable (see the playground on the homepage).
- The `trace` sink is **free in production** (ADR-0027 INV-2): no overhead when
  no driver is attached.

## Release mode — codegen'd native

In release, the same IR is **codegen'd** to idiomatic Swift/SwiftUI and
Kotlin/Jetpack Compose. There is no VM, no patch protocol, no shadow tree — the
output is a normal native app.

<CardGrid>
  <Card title="SwiftUI" icon="seti:swift">
    `component Counter` → `struct Counter: View`. `state` → `@State`.
    `Column(gap:)` → `VStack(spacing:)`.
  </Card>
  <Card title="Jetpack Compose" icon="seti:kotlin">
    `component Counter` → `@Composable fun Counter()`. `state` →
    `remember { mutableStateOf(...) }`. `Column(gap:)` →
    `Column(verticalArrangement = spacedBy(...))`.
  </Card>
</CardGrid>

## Why two modes?

The dev loop needs liveness (patch, don't recompile); the shipped product needs
zero runtime overhead (native codegen). Keeping a single IR as the contract means
the dev experience and the release binary are provably the same program — which
is exactly what the parity trace tool (FLUX-023) proves.

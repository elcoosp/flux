---
title: Showcase apps
description: The real Flux example apps that double as integration tests — counter, todo, and router.
---

Flux ships a small set of **showcase apps** under `examples/`. They are more than
demos: each is a vertical slice that exercises the real reactive VM, the wire
protocol, and the dev/release tiers, and they double as integration tests in CI.

## counter

`examples/counter/` — the canonical vertical slice.

```flux
compo Counter
  state count: Int = 0
  Column gap: 8.0
    Text text: "tapped {count} times"
    Button text: "Increment", onPress: || { count = count + 1 }
```

Used by the headless full-pipeline e2e test
(`crates/flux-devserver/tests/full_pipeline.rs`) and the recommended integration
path (ADR-0036). Walk it end to end in the
[Counter example](/guides/counter-example/).

## todo

`examples/todo/` — a real app exercising every MLP primitive plus capabilities.

- Components: `Text`, `Button`, `TextInput`, `Column`, `Row`, `Image`,
  `Router`, `Screen`.
- `TextInput` is **controlled**: `onChangeText: |t| { newTask = t }` writes the
  typed value into the `newTask` signal, so "Add task" can read it.
- `Router.navigate` (cap 3 / method 1) drives the visible screen via signal 97.
- `Storage.removeItem` (cap 2 / method 3) is a real `CALL_CAP` on Reset.

> **List-modeling note (read before porting a real todo):** the MLP lower pass
> emits a `ForEach` as an empty splice and handler `WRITE_SIGNAL` targets are
> statically-fixed signal names with no list set/remove/slice ops. A
> dynamically-sized list is **not** expressible in the current dev pipeline, so
> this example models the list as five **fixed** slots (each a `String` + `Bool`
> signal). "Add task" fills the first empty slot; "Toggle"/"Remove" flip or clear
> a slot's signals. This is the honest MLP shape today.

## router

`examples/router/` — demonstrates `Router` + `Screen` navigation and the
`route` prop the reconcilers switch on (FNV-1a `"route"`).

## Using the showcases as tests

Each example builds under `flux build` (release codegen → Swift/Kotlin) and runs
a smoke path in CI. They are the living proof that the stdlib surface in the
[Cookbook](/guides/cookbook/) maps to real, compiling apps. When you add a
primitive, add a showcase usage so the coverage check keeps the docs honest.

---
title: The Counter example
description: Walk through the canonical Flux counter — state, a bound Text, a Button tap — in dev and in both release codegen targets (SwiftUI and Jetpack Compose).
---

This is the smallest interesting Flux program. It mirrors the real file shipped
at `examples/counter/main.flux`, and the homepage playground's recorded trace is
built from the same shape.

```flux
compo Counter
  $count: Int = 0

  Column gap: 8.0
    Text text: "tapped {count} times"
    Button text: "Increment", onPress: || { count = count + 1 }
```

`flux build ios` on this exact file emits the Swift below; `flux build android`
emits the Kotlin. Both are reproducible — run them yourself from
`examples/counter/`.

## What each line means

- `$count: Int = 0` — a mutable signal cell (the `$` sigil marks it as a signal).
  The **host** owns `count`; the server only ever sees its type and initial
  value. This is [host-authoritative state](/concepts/host-authoritative-state/).
- `Text(text: "tapped {count} times")` — interpolates the signal into a string
  literal. This node's `signal_deps` is exactly the id of `count`.
- `Button(text: "Increment", onPress: || { count = count + 1 })` — registers an
  `onPress` handler closure (a `Handler` with no arguments) whose body writes
  `count`. The closure is shipped to the host as bytecode (Appendix D §D.8) and
  run in the host VM on tap.

## What happens on a tap

1. The `onPress` closure runs in the host VM, executing
   `READ_SIGNAL count` → `LOAD_INT_CONST` → `ADD_I64` → `WRITE_SIGNAL count`.
2. The VM reports the written signal id(s), sorted ascending, as the
   `signals` trace event.
3. The host intersects those signals with each node's `signal_deps`: only the
   `Text` reads `count`, so the `dirty` set is exactly that one node.
4. The `Text` re-materializes its props and fires `update` — **one** update,
   **zero** builds. The `Column` and `Button` are untouched
   ([skip_unchanged](/concepts/host-authoritative-state/)).

Step through this exact scenario on the [homepage playground](/).

## Release codegen

The dev path interprets the IR and patches it. A release build instead codegen's
the same IR to native source. Running `flux build ios` on the file above produces:

```swift
struct Counter: View {
    @State private var count: Int = 0
    var body: some View {
  VStack {
      Text("tapped \(count) times")
      Button(action: { count = (count + 1) }) {
          Text("Increment")
      }
  }
    }
}
```

`flux build android` produces:

```kotlin
@Composable fun Counter(
) {
    var count by remember { mutableStateOf<Int>(0) }
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("tapped ${count} times")
        Button(onClick = { count = (count + 1) }) {
            Text("Increment")
        }
    }
}
```

The `compo` → `struct`/`@Composable`, `$count` → `@State`/`remember {
mutableStateOf }`, and `Column(gap:)` → `VStack(spacing:)` /
`Column(spacedBy(...))` mappings are exactly the
[Dev vs Release](/concepts/dev-vs-release/) contract. The two outputs are the same
program — which is what the parity tooling proves.

## Try it yourself

```bash
# from the repo root
cd examples/counter
flux dev                 # WebSocket on :7331

# in another shell, render the release forms:
flux build ios           # -> platforms/ios/Generated/main.swift
flux build android       # -> platforms/android/Generated/main.kt
```

Then run the host app against the dev server (see the
[Quickstart](/guides/quickstart/)) to watch the `Button` tap hot-reload the tree.

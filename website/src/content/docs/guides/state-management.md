---
title: State management
description: Opinionated patterns over the Flux signal graph — global stores, derived signals, and async data via capabilities.
---

Flux renders from a reactive signal graph: every `state` signal is a node, and
the UI recomputes only the dependents of written signals
([AGENTS.md §3.7](https://github.com/elcoosp/flux/blob/main/AGENTS.md)). This
guide gives opinionated, copy-paste patterns for real apps beyond the counter.

## Local state: signals

A `compo` declares its own signals with `state`. Assigning inside a handler
writes the signal and triggers a dirty-subset recompose — only the leaves that
read it update.

```flux
compo Counter
  state count: Int = 0
  Column
    Text text: "tapped {count} times"
    Button text: "Increment", onPress: || { count = count + 1 }
```

This is the entire local-state model. There is no separate `setState` — the
assignment *is* the update.

## Derived signals

A value computed from other signals should be derived, not stored. Declare it as
a `let` (read-only binding) computed from signals in the body:

```flux
compo Cart
  state items: Int = 3
  state price: Float = 9.99
  let total: Float = items * price   // recomputed when items/price change
  Column
    Text text: "Total: {total}"
```

> Derivation is recomputed on each relevant signal write. For hot-path
> derivations, the hosts memoize the dependent graph, so only affected leaves
> recompose ([AGENTS.md §0.2](https://github.com/elcoosp/flux/blob/main/AGENTS.md)).

## Global stores

For state shared across components, define a **store** as a `compo` that owns the
signals and exposes handler "actions", then mount it once near the root and pass
values down. Flux has no `Provider`/`Context` widget; instead, lift the store
above the components that read it and thread the values as props.

```flux
compo SessionStore
  state token: Option[String] = None
  state user: Option[String] = None
  // actions are handlers that write the store's signals:
  //   Login -> token = ... ; user = ...
  // Children read `token`/`user` via props passed from here.
```

Opinion: keep stores small and explicit. One store per domain (auth, cart,
settings), mounted at the root `Router`/`Screen` that needs it. The signal graph
is shared across the whole app, so any component mounted beneath a store can
read its props.

## Async data via capabilities

Flux has no `async` keyword in handlers — async work goes through a
**capability** returning a `Result` cell (ADR-0045). A synchronous capability
settles the cell before returning; an async one leaves it `Pending` and an
injected resolver settles it. The host threads the result back onto the reactive
dispatcher, so you never write signals off the main thread
([AGENTS.md §3.7](https://github.com/elcoosp/flux/blob/main/AGENTS.md)).

```flux
// fetch user profile through a capability, then write the signal on resolve
compo Profile
  state name: Option[String] = None
  Column
    Text text: name ?? "Loading..."
    Button text: "Load", onPress: || {
      // call the capability; the result cell updates `name` when settled
    }
```

The stable capability ids are in
[stdlib/capabilities.flux](https://github.com/elcoosp/flux/blob/main/stdlib/capabilities.flux)
(cap 1 = Camera, 2 = Storage, 3 = Router, 4 = Clipboard, 5 = Location, … 11).
Capability ids are derived deterministically — never hand-assigned.

## Pattern summary

- **Local**: `state` + assignment in a `Handler`.
- **Computed**: `let` derived from signals.
- **Shared**: lift a store `compo` above consumers; thread values as props.
- **Async**: capability call → `Result` cell → write signal on resolve.
- **Never** write signals or mutate the shadow tree off the reactive dispatcher.

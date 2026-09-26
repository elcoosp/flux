# SIDECAR: `Router.initialRouteName` is NOT dead (T-604.17)

## Status
**Rejected** — `initialRouteName` is used by codegen (T-403.7).

## Detail
T-604.17 instructions said `Router.initialRouteName` is dead and should be
removed. Inspection of the codegen lowerer shows it is actively read:

- `crates/flux-codegen-core/src/emitter.rs:667-671` — the Kotlin backend
  derives `startDestination` from the `initialRouteName` prop.
- `crates/flux-codegen-kotlin/src/backend_impl.rs:76` — the comment
  "derive startDestination from initialRouteName prop" matches.

The comment in `stdlib/router.flux` was updated to reflect that
`initialRouteName` is the T-403.7 mechanism (not dead), preserving the prop.
No removal was performed.

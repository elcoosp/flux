// router.flux — `Router` + `Screen` adapter components (Appendix F.6 / F.7).
//
// `Router` owns a stack of `Screen` children and drives platform navigation.
// `Screen(route)` wraps a single content child and is addressed by its `route`
// string (Appendix F.7). Screen state is preserved across push/pop per the
// runtime reconciler contract.
//
// The new indentation-delimited surface syntax (FLUX-00X) requires a view call
// to carry at least one named prop in order to own an indented child block, so
// `Router` declares an `initialRouteName` route prop (the host still falls back
// to signal 97 / the first Screen) and `Screen` takes its `route` as a named
// prop — the exact prop the iOS / Android reconcilers read via `FNV-1a("route")`
// to pick the visible screen (ADR-0045).
//
// Native rendering is defined by Appendix F.6/F.7 (UINavigationController /
// FrameLayout stack in dev mode; SwiftUI `NavigationStack(path:)` /
// Compose `NavHost` in release).

compo Router(initialRouteName: String = "home")
  // Adapter container — children are `Screen` instances.

// Each screen carries a stable `route` string used as its route key. The host
// reads this prop via `FNV-1a("route")`, so the prop name MUST be `route`.
compo Screen(route: String)
  // Adapter container — single child is the screen content.

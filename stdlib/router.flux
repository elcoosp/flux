// router.flux — `Router` + `Screen` adapter components (Appendix F.6 / F.7).
//
// `Router` owns a stack of `Screen` children and drives platform navigation.
// It takes no props (Appendix F.6). `Screen(name)` wraps a single content
// child and is addressed by its string name (Appendix F.7). Screen state is
// preserved across push/pop per the runtime reconciler contract.
//
// Native rendering is defined by Appendix F.6/F.7 (UINavigationController /
// FrameLayout stack in dev mode; SwiftUI `NavigationStack(path:)` /
// Compose `NavHost` in release).

compo Router()
  // Adapter container — children are `Screen` instances.

// `Screen` is declared in this module because Appendix F.7 shows it nested
// under the Router contract (the nav grammar pairs `Router { Screen(..) }`).
// Each screen carries a stable string name used as its route key.
compo Screen(name: String)
  // Adapter container — single child is the screen content.

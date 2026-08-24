// platform.flux — platform primitives used by user modules.
//
// This module re-exports the platform conditionals and context helpers that
// user components rely on. Per Appendix B.3.8 a `platform` value of type
// String is in scope for platform-conditional rendering, and per B.3.5
// `useContext(RouterContext)` yields a router handle with `navigate`.
//
// These are declarations of values provided by the runtime / prelude:
//   platform        String        current platform tag ("ios" | "android" | "web")
//   RouterContext   Context       context type carrying the active router
//   RouterHandle    (context)     value returned by useContext(RouterContext)
//
// `RouterHandle.navigate(target: String) -> Unit` performs the platform
// navigation transition. The spelling mirrors Appendix B.3.5
// (`router.navigate("profile")`).

// Current platform tag. Bound by the host at boot; read-only.
state platform: String = "ios"

// Opaque router context value. `navigate` drives the navigation stack.
type RouterHandle = RouterHandle

fn navigate(handle: RouterHandle, target: String) -> Unit {
  // Provided by the runtime: pushes/pops screens on the active Router.
}

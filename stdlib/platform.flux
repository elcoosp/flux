// platform.flux — platform primitives used by user modules.
//
// This module re-exports the platform conditional that user components rely on.
// Per Appendix B.3.8 a `platform` value of type String is in scope for
// platform-conditional rendering.
//
// These are declarations of values provided by the runtime / prelude:
//   platform()      String        current platform tag ("ios" | "android" | "web")
//
// Navigation is no longer a hand-written `fn` here — it is the `Router.navigate`
// capability (see stdlib/capabilities.flux). Both host reconcilers present only
// the child `Screen` whose `route` prop equals the active navigation target
// (ADR-0045).

// Current platform tag. Bound by the host at boot; read-only. Queried by
// calling `platform()` in platform-conditional rendering (Appendix B.3.8
// writes `if platform() == "ios" { … }`). Declared as a `fn`, not a module
// `state`, so there is no file-scope state form in the language.
fn platform() -> String {
  // Provided by the runtime: returns the host's platform tag.
}

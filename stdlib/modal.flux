// modal.flux — `Modal` adapter compo (FLUX-038).
//
// Centered modal presented over a dimmed scrim; dismisses on tap-outside.
// Overlay container: `content` is its `Modal`'s children. The presentation is
// data the host consumes (a named transition mapped to the native equivalent —
// iOS `.fullScreenCover` / Compose `Dialog`); it is never a wire animation
// frame (see ADR-0047 + FLUX-038).
//
// Props follow the PRD-N overlay-family contract:
//   onDismiss  Handler        invoked when the host presentation is dismissed
//
// Host adapter wiring (the native surface + transition) is gated on the
// ADR-0048 iOS dev-tier convergence decision; the type-checker and codegen can
// name and emit the primitive today, and the parity trace test pins the
// dev/release mapping.

compo Modal(
  onDismiss: Handler = fn() {},
)
  // Overlay container — `content` children supplied by callers.

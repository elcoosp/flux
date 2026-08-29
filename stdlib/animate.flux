// animate.flux — `Animate` adapter compo (FLUX-042).
//
// Signal-graph animation wrapper: drives `signal` through a spring/timing
// `curve` while rendering `content`. The curve is data the host consumes — the
// host maps it to its native animation API (SwiftUI `withAnimation(spec)`,
// Compose `withAnimation`) wrapping the child subtree; animation is never
// shipped as frames on the wire (ADR-0047 + FLUX-042).
//
// Props follow the PRD-N motion-family contract:
//   signal    Signal          the signal the curve drives (primary prop)
//   curve     String          named curve: "spring" | "easeIn" | "easeOut"
//                                | "easeInOut" | "linear", defaults to "easeInOut"
//   duration  Float           duration in seconds for timing curves, defaults to 0.3
//
// Host adapter wiring (the native `withAnimation` surface) is gated on the
// ADR-0048 iOS dev-tier convergence decision; the type-checker and codegen can
// name and emit the primitive today, and parity reduces both backends'
// `withAnimation` to `Animate`.

compo Animate(
  signal: Signal,
  curve: String = "easeInOut",
  duration: Float = 0.3,
)
  // Motion wrapper — `content` children supplied by callers.

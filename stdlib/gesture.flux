// gesture.flux — `Gesture` adapter compo (FLUX-041, Appendix F gesture family).
//
// A wrapper that attaches one gesture recognizer to its child subtree. `kind`
// selects the recognizer: "longPress" | "swipe" | "drag" | "pinch". `onGesture`
// fires when the gesture is recognized (reuses the `onClick` handler contract).
// `threshold` is the activation delta for continuous gestures (drag/pinch).
//
// Native rendering: SwiftUI `UIGestureRecognizer` family / Compose
// `Modifier.pointerInput` (release); the dev host maps the same node kind
// through `GestureAdapter`, which reconciles children by stable node id and
// declares the gesture intent as view properties (Appendix F, ADR-0047).

compo Gesture(
  kind: String = "longPress",
  onGesture: Handler = fn() {},
  threshold: Float = 0.0,
)
  // Overlay container — child subtree supplied by callers.

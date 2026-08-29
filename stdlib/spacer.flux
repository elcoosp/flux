// spacer.flux — `Spacer` adapter compo (FLUX-037).
//
// An elastic gap that expands to fill available space along the parent's main
// axis. Props follow the PRD-N layout-family contract:
//   flex      Float          relative grow weight, defaults to 1.0
//
// A `Spacer` carries no children. Native rendering is defined by the FLUX-037
// design: SwiftUI `Spacer()` (release) / Compose `Spacer` (release), and the
// dev adapter on each platform maps to the equivalent spring.

compo Spacer(
  flex: Float = 1.0,
)
  // Leaf adapter — no children.

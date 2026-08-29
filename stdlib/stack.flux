// stack.flux — `Stack` adapter compo (FLUX-037).
//
// Z-order overlay container: children are stacked above one another, the last
// child painted on top. Props follow the PRD-N layout-family contract:
//   gap        Float          spacing between stacked children, defaults to 0.0
//   alignment  Option[Alignment] cross-axis alignment, defaults to None
//
// The `= 0.0` / `= None` defaults encode optional props per Appendix B.2
// (the gap-default form was closed by FLUX-003 and verified by FLUX-015's
// parse check). Children are laid out as an overlay; native rendering is
// defined by the FLUX-037 design: SwiftUI `ZStack(spacing:)` in release,
// Compose `Box` in release, and the dev adapter on each platform maps to the
// equivalent overlay container.

compo Stack(
  gap: Float = 0.0,
  alignment: Option[Alignment] = None,
)
  // Adapter container — children supplied by callers.

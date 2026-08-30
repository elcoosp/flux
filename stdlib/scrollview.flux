// scrollview.flux — `ScrollView` adapter compo (FLUX-056, PRD-N).
//
// A scrollable viewport for a single scrollable child subtree. Props follow the
// PRD-N layout-family contract:
//   orientation   Option[String]   scroll axis: "vertical" (default) or
//                                  "horizontal"; absent means "vertical"
//
// Children supplied by callers are carried inside the scrollable content.
// Native rendering is defined by the FLUX-056 design: SwiftUI `ScrollView`
// (release) / Android `ScrollView` (release), and the dev adapter on each
// platform maps to the equivalent scrollable container.

compo ScrollView(
  orientation: Option[String] = None,
)
  // Adapter container — children supplied by callers.

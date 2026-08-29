// safearea.flux — `SafeArea` adapter compo (FLUX-037).
//
// Insets its children within the platform's safe-area (status bar, home
// indicator, notches). Props follow the PRD-N layout-family contract:
//   edges     Option[String] which edges to inset (e.g. "top", "bottom"),
//                                defaults to None (all edges)
//
// Children are laid out within the safe area. Native rendering is defined by
// the FLUX-037 design: SwiftUI `SafeArea` (release) / Compose `Scaffold`
// content padding (release), and the dev adapter on each platform maps to the
// equivalent insetting container.

compo SafeArea(
  edges: Option[String] = None,
)
  // Adapter container — children supplied by callers.

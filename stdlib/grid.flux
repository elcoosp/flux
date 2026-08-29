// grid.flux — `Grid` adapter compo (FLUX-037).
//
// Two-dimensional responsive grid of children. Props follow the PRD-N
// layout-family contract:
//   columns   Int            target column count, defaults to 2
//   gap       Float          spacing between cells, defaults to 0.0
//
// Native rendering is defined by the FLUX-037 design: SwiftUI `Grid` (release)
// / Compose `LazyVerticalGrid` (release), and the dev adapter on each platform
// maps to the equivalent grid container. Defaults encode optional props per
// Appendix B.2.

compo Grid(
  columns: Int = 2,
  gap: Float = 0.0,
)
  // Adapter container — children supplied by callers.

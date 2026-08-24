// column.flux — `Column` adapter component (Appendix F.3).
//
// Vertical stack. Props follow the Appendix F.3 contract exactly:
//   gap        Float          spacing between children, defaults to 0.0
//   alignment  Option[Alignment] cross-axis alignment, defaults to None
//
// The `= 0.0` / `= None` defaults encode Appendix F.3's optional props
// (see ADR stdlib-grammar-gaps, G2).
//
// Children are laid out vertically. Native rendering is defined by
// Appendix F.3 (UIStackView(axis: .vertical) / LinearLayout(VERTICAL) in
// dev mode; SwiftUI `VStack(spacing:)` / Compose `Column(spacing:)` in release).

component Column(
  gap: Float = 0.0,
  alignment: Option[Alignment] = None,
) {
  // Adapter container — children supplied by callers.
}

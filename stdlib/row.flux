// row.flux — `Row` adapter component (Appendix F.4).
//
// Horizontal stack. Props follow the Appendix F.4 contract exactly:
//   gap        Float          spacing between children, defaults to 0.0
//   alignment  Option[Alignment] cross-axis alignment, defaults to None
//
// The `= 0.0` / `= None` defaults encode Appendix F.4's optional props
// (see ADR stdlib-grammar-gaps, G2).
//
// Children are laid out horizontally. Native rendering is defined by
// Appendix F.4 (UIStackView(axis: .horizontal) / LinearLayout(HORIZONTAL)
// in dev mode; SwiftUI `HStack(spacing:)` / Compose `Row(spacing:)` in release).

component Row(
  gap: Float = 0.0,
  alignment: Option[Alignment] = None,
) {
  // Adapter container — children supplied by callers.
}

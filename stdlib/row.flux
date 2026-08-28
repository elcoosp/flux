// row.flux — `Row` adapter compo (Appendix F.4).
//
// Horizontal stack. Props follow the Appendix F.4 contract exactly:
//   gap        Float          spacing between children, defaults to 0.0
//   alignment  Option[Alignment] cross-axis alignment, defaults to None
//
// The `= 0.0` / `= None` defaults encode Appendix F.4's optional props.
// `prop_decl` carries an optional `"=" expr` default (Appendix B.2); the gap
// recorded as G2 in ADR-0037-stdlib-grammar-gaps was closed by FLUX-003 and is
// verified by FLUX-015's parse check.
//
// Children are laid out horizontally. Native rendering is defined by
// Appendix F.4 (UIStackView(axis: .horizontal) / LinearLayout(HORIZONTAL)
// in dev mode; SwiftUI `HStack(spacing:)` / Compose `Row(spacing:)` in release).

compo Row(
  gap: Float = 0.0,
  alignment: Option[Alignment] = None,
)
  // Adapter container — children supplied by callers.

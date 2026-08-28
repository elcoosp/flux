// column.flux — `Column` adapter compo (Appendix F.3).
//
// Vertical stack. Props follow the Appendix F.3 contract exactly:
//   gap        Float          spacing between children, defaults to 0.0
//   alignment  Option[Alignment] cross-axis alignment, defaults to None
//
// The `= 0.0` / `= None` defaults encode Appendix F.3's optional props.
// `prop_decl` carries an optional `"=" expr` default (Appendix B.2); the gap
// recorded as G2 in ADR-0037-stdlib-grammar-gaps was closed by FLUX-003 and is
// verified by FLUX-015's parse check.
//
// Children are laid out vertically. Native rendering is defined by
// Appendix F.3 (UIStackView(axis: .vertical) / LinearLayout(VERTICAL) in
// dev mode; SwiftUI `VStack(spacing:)` / Compose `Column(spacing:)` in release).

compo Column(
  gap: Float = 0.0,
  alignment: Option[Alignment] = None,
)
  // Adapter container — children supplied by callers.

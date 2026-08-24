// button.flux — `Button` adapter component (Appendix F.2).
//
// Props follow the Appendix F.2 contract exactly:
//   text      String   required
//   onClick   Handler  required — fired on tap
//   enabled   Bool     defaults to true when omitted
//   color     Option[Color] defaults to None when omitted
//
// The `= true` / `= None` defaults encode Appendix F.2's optional props.
// `prop_decl` carries an optional `"=" expr` default (Appendix B.2); the gap
// recorded as G2 in ADR stdlib-grammar-gaps was closed by FLUX-003 and is
// verified by FLUX-015's parse check.
//
// Native rendering is defined by Appendix F.2 (UIButton / android.widget.Button
// in dev mode; SwiftUI `Button` / Compose `Button` in release).

component Button(
  text: String,
  onClick: Handler,
  enabled: Bool = true,
  color: Option[Color] = None,
) {
  // Adapter leaf — native rendering defined by Appendix F.2.
}

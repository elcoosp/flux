// text.flux — `Text` adapter compo (Appendix F.1).
//
// Renders a string. Props follow the Appendix F.1 contract exactly:
//   text        String          required
//   font        Option[Font]    defaults to None when omitted
//   size        Option[Float]   defaults to None when omitted
//   color       Option[Color]   defaults to None when omitted
//   alignment   Option[Alignment] defaults to None when omitted
//   maxLines    Option[Int]     defaults to None when omitted
//   overflow    Option[Overflow]  defaults to None when omitted
//
// The `= None` defaults encode Appendix F.1's "optional" props. `prop_decl`
// carries an optional `"=" expr` default (Appendix B.2); the gap recorded as
// G2 in ADR-0037-stdlib-grammar-gaps was closed by FLUX-003 and is verified by
// FLUX-015's parse check.
//
// Native rendering is defined by Appendix F.1 (UILabel / TextView in dev
// mode; SwiftUI `Text` / Compose `Text` in release). This declaration is
// the API contract; the body is supplied natively.

compo Text(
  text: String,
  font: Option[Font] = None,
  size: Option[Float] = None,
  color: Option[Color] = None,
  alignment: Option[Alignment] = None,
  maxLines: Option[Int] = None,
  overflow: Option[Overflow] = None,
)
  // Adapter leaf — native rendering defined by Appendix F.1.

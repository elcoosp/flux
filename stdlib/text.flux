// text.flux — `Text` adapter component (Appendix F.1).
//
// Renders a string. Props follow the Appendix F.1 contract exactly:
//   text        String          required
//   font        Option[Font]    defaults to None when omitted
//   size        Option[Float]   defaults to None when omitted
//   color       Option[Color]   defaults to None when omitted
//   alignment   Option[Alignment] defaults to None when omitted
//   max_lines   Option[Int]     defaults to None when omitted
//   overflow    Option[Overflow]  defaults to None when omitted
//
// The `= None` defaults encode Appendix F.1's "optional" props; the parser
// support for default values is tracked in ADR stdlib-grammar-gaps (G2).
//
// Native rendering is defined by Appendix F.1 (UILabel / TextView in dev
// mode; SwiftUI `Text` / Compose `Text` in release). This declaration is
// the API contract; the body is supplied natively.

component Text(
  text: String,
  font: Option[Font] = None,
  size: Option[Float] = None,
  color: Option[Color] = None,
  alignment: Option[Alignment] = None,
  max_lines: Option[Int] = None,
  overflow: Option[Overflow] = None,
) {
  // Adapter leaf — native rendering defined by Appendix F.1.
}

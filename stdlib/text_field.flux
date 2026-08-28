// text_field.flux — `TextField` adapter compo (Appendix F.5).
//
// Props follow the Appendix F.5 contract exactly:
//   text        String          controlled value, defaults to ""
//   onChange    Handler         fired on every text change
//   placeholder Option[String]  hint text, defaults to None
//   ref         Option[Ref[TextField]] native view handle, defaults to None
//   enabled     Bool            editable, defaults to true
//   secure      Bool            password masking, defaults to false
//   keyboard    Option[KeyboardType] soft keyboard flavor, defaults to None
//
// The `= ""` / `= true` / `= false` / `= None` defaults encode Appendix F.5's
// optional props. `prop_decl` carries an optional `"=" expr` default
// (Appendix B.2); the gap recorded as G2 in ADR-0037-stdlib-grammar-gaps was closed
// by FLUX-003 and is verified by FLUX-015's parse check.
//
// Native rendering is defined by Appendix F.5 (UITextField / EditText in
// dev mode; SwiftUI `TextField` / Compose `TextField` in release).

compo TextField(
  text: String = "",
  onChange: Handler,
  placeholder: Option[String] = None,
  ref: Option[Ref[TextField]] = None,
  enabled: Bool = true,
  secure: Bool = false,
  keyboard: Option[KeyboardType] = None,
)
  // Adapter leaf — native rendering defined by Appendix F.5.

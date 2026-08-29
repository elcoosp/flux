// text_field.flux — `TextInput` adapter compo (Appendix F.5).
//
// Props follow the Appendix F.5 contract exactly:
//   text           String          controlled value, defaults to ""
//   onChangeText   Handler         fired on every text change (RN TextInput verb)
//   placeholder    Option[String]  hint text, defaults to None
//   ref            Option[Ref[TextInput]] native view handle, defaults to None
//   enabled        Bool            editable, defaults to true
//   secureTextEntry Bool           password masking, defaults to false
//   keyboardType   Option[KeyboardType] soft keyboard flavor, defaults to None
//
// The `= ""` / `= true` / `= false` / `= None` defaults encode Appendix F.5's
// optional props. `prop_decl` carries an optional `"=" expr` default
// (Appendix B.2); the gap recorded as G2 in ADR-0037-stdlib-grammar-gaps was
// closed by FLUX-003 and is verified by FLUX-015's parse check.
//
// Native rendering is defined by Appendix F.5 (UITextField / EditText in
// dev mode; SwiftUI `TextField` / Compose `TextField` in release).

compo TextInput(
  text: String = "",
  onChangeText: Handler,
  placeholder: Option[String] = None,
  ref: Option[Ref[TextInput]] = None,
  enabled: Bool = true,
  secureTextEntry: Bool = false,
  keyboardType: Option[KeyboardType] = None,
)
  // Adapter leaf — native rendering defined by Appendix F.5.

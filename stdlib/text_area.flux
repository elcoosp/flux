// text_area.flux — `TextArea` adapter compo (FLUX-040, Appendix F form family).
//
// Multi-line editable text. The `value` signal is the controlled string;
// `onChange` fires with the new string on every edit (same contract as
// `TextInput`). `placeholder` is the hint shown when empty, `maxLines` caps the
// visible height, and `enabled` gates editing.
//
// Native rendering: SwiftUI `TextEditor` / Compose `TextField` (multi-line,
// release); the dev host maps the same node kind through `TextAreaAdapter`
// (Appendix F, ADR-0047).

compo TextArea(
  value: String = "",
  onChange: Handler = fn() {},
  placeholder: Option[String] = None,
  maxLines: Option[Int] = None,
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

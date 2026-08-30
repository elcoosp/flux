// date_picker.flux — `DatePicker` adapter compo (FLUX-040, Appendix F form family).
//
// Date selector. The `value` signal is the controlled epoch-millis integer;
// `onChange` fires with the new value when the user confirms a date. `min`/
// `max` bound the selectable range; `enabled` gates interaction.
//
// Native rendering: SwiftUI `DatePicker` / Compose `DatePickerDialog` (release);
// the dev host maps the same node kind through `DatePickerAdapter` (Appendix F,
// ADR-0047).

compo DatePicker(
  value: Int = 0,
  onChange: Handler = fn() {},
  min: Int = 0,
  max: Int = 0,
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

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
  // Audit fix: previously `min: Int = 0` / `max: Int = 0` — a default
  // selectable range of exactly one millisecond (1970-01-01T00:00:00Z), with
  // an undocumented host special-case that treated 0 as "unbounded".
  // `Option[Int]` makes the "unbounded" default explicit and removes the
  // sentinel-value ambiguity.
  min: Option[Int] = None,
  max: Option[Int] = None,
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

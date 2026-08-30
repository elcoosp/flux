// picker.flux — `Picker` adapter compo (FLUX-040, Appendix F form family).
//
// Single-selection control. The `value` signal is the controlled selected
// index; `onChange` fires with the new index when the user picks an option.
// `items` is the candidate list; `enabled` gates interaction.
//
// Native rendering: SwiftUI `Picker` / Compose `DropdownMenu` (release); the
// dev host maps the same node kind through `PickerAdapter` (Appendix F, ADR-0047).

compo Picker(
  value: Int = 0,
  onChange: Handler = fn() {},
  items: List[String] = [],
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

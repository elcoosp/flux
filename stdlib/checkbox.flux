// checkbox.flux — `Checkbox` adapter compo (FLUX-040, Appendix F form family).
//
// Boolean selection box. The `value` signal is the controlled state; `onChange`
// fires with the new boolean when the user toggles it. An optional `label`
// renders beside the box; `enabled` gates interaction.
//
// Native rendering: SwiftUI `Toggle` / Compose `Checkbox` (release); the dev
// host maps the same node kind through `CheckboxAdapter` (Appendix F, ADR-0047).

compo Checkbox(
  value: Bool = false,
  onChange: Handler = fn() {},
  label: Option[String] = None,
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

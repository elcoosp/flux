// switch.flux — `Switch` adapter compo (FLUX-040, Appendix F form family).
//
// Two-state toggle. The `value` signal is the controlled state; `onChange`
// fires with the new boolean when the user flips it (same contract as
// `TextInput`). `enabled` gates interaction.
//
// Native rendering: SwiftUI `Toggle` / Compose `Switch` (release); the dev host
// maps the same node kind through `SwitchAdapter` (Appendix F, ADR-0047).

compo Switch(
  value: Bool = false,
  onChange: Handler = fn() {},
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

// toggle.flux — `Toggle` adapter compo (FLUX-077, Appendix F form family).
//
// Two-state toggle. The `value` signal is the controlled state; `onValueChange`
// fires with the new boolean when the user toggles it (same contract as
// `Switch` and `TextInput`). `enabled` gates interaction.
//
// Native rendering: SwiftUI `Toggle` / Compose `Switch` (release); the dev host
// maps the same node kind through `ToggleAdapter` (Appendix F, ADR-0047).
// The callback prop is `onValueChange` — shared by both kits
// (see `examples/todo` and the codegen bridge).

compo Toggle(
  value: Bool = false,
  onValueChange: Handler = fn() {},
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-077.

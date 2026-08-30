// slider.flux — `Slider` adapter compo (FLUX-040, Appendix F form family).
//
// Continuous-value selector. The `value` signal is the controlled float;
// `onChange` fires with the new float as the user drags the thumb. `min`/`max`
// bound the range, `step` quantizes it, and `enabled` gates interaction.
//
// Native rendering: SwiftUI `Slider` / Compose `Slider` (release); the dev host
// maps the same node kind through `SliderAdapter` (Appendix F, ADR-0047).

compo Slider(
  value: Float = 0.0,
  onChange: Handler = fn() {},
  min: Float = 0.0,
  max: Float = 1.0,
  step: Float = 0.0,
  enabled: Bool = true,
)
  // Adapter leaf — native rendering defined by FLUX-040.

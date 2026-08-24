// font.flux — `Font` type and platform presets (mlp-spec §18.6).
//
// `Font` is a single-variant algebraic data type carrying a family string,
// a point size, a weight, and a slant. Using a positional variant
// `Font(family, size, weight, style)` (rather than a record literal) keeps
// the constant bindings within the Appendix B grammar that is already
// exercised by the §18.6 examples; record-literal construction in value
// position is tracked in ADR stdlib-grammar-gaps (G3). The three presets
// (`Font.body`, `Font.title`, `Font.caption`) map onto the platform's
// built-in text styles per §18.6.

type Font =
  | Font(String, Float, Weight, Style)

// Relative font weight (platform spelling: thin..bold).
type Weight =
  | Thin
  | Light
  | Regular
  | Medium
  | Bold
  | Heavy

// Slant of the glyphs.
type Style =
  | Normal
  | Italic

// Platform text-style presets (in scope as `Font.body`, etc. via the
// implicit prelude). `family` is empty so codegen selects the platform
// default body/title/caption style; `weight` is Regular and `style` Normal.
Font.body    = Font("", 17.0, Regular, Normal)
Font.title   = Font("", 28.0, Bold,    Normal)
Font.caption = Font("", 12.0, Regular, Normal)

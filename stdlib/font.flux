// font.flux — `Font` type and platform presets (mlp-spec §18.6).
//
// `Font` is a single-variant algebraic data type carrying a family string,
// a point size, a weight, and a slant. The constants use positional variant
// construction `Font(family, size, weight, style)`, the form the §18.6
// examples show for `RGB(..)`. The record-literal alternative recorded as G3
// in ADR stdlib-grammar-gaps is also a grammar production now (`record_lit`,
// Appendix B.2), so the choice here is stylistic rather than forced. The three
// presets (`Font.body`, `Font.title`, `Font.caption`) map onto the platform's
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

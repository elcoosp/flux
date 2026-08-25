// color.flux — `Color` type and built-in constants (mlp-spec §18.6).
//
// `Color` is an algebraic data type with a single `RGB` variant carrying
// three Float channels in the 0.0–1.0 range. The named constants below are
// provided by the stdlib per §18.6 and mirror the documented spelling
// `Color.red`, `Color.green`, `Color.blue`, `Color.black`, `Color.white`.
//
// The constant bindings (`Color.red = RGB(..)`) use the
// top-level `Name.field = expr` form shown in §18.6. That form is now a
// grammar production (`const_binding`, Appendix B.2): the gap recorded as G1
// in ADR-0037-stdlib-grammar-gaps was closed by FLUX-003 and is verified by
// FLUX-015's parse check. The values themselves are fully within Appendix B
// (a 3-tuple variant constructor).

type Color =
  | RGB(Float, Float, Float)

// Named constants (in scope as `Color.red`, etc. via the implicit prelude).
Color.red   = RGB(1.0, 0.0, 0.0)
Color.green = RGB(0.0, 1.0, 0.0)
Color.blue  = RGB(0.0, 0.0, 1.0)
Color.black = RGB(0.0, 0.0, 0.0)
Color.white = RGB(1.0, 1.0, 1.0)

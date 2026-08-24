// traits.flux — stdlib type-class traits (mlp-spec §18.2, Appendix B.3.2).
//
// Flux traits are Haskell-style type classes (not Rust traits). They are
// declared with `trait` and resolved by the type checker at FLUX-012. These
// three are part of the default prelude (mlp-spec §18.3 stdlib-traits row).
//
// Method signatures follow the §18.2 / B.3.2 spelling. The spec body shows
// `trait Numeric[T]` with `fn zero()`, `fn one()`, `fn +(a: T, b: T)`,
// `fn -(a: T, b: T)`; the forms below record the additive group plus `Eq`
// and `Show` per the §18.2 enumerations. Operator methods are declared by
// their symbolic name (the parser treats `+` / `-` / `==` / `!=` as method
// identifiers of a trait — see ADR stdlib-grammar-gaps, G4).

trait Numeric[T] {
  fn zero() -> T
  fn one() -> T
  fn +(a: T, b: T) -> T
  fn -(a: T, b: T) -> T
}

trait Eq[T] {
  fn ==(a: T, b: T) -> Bool
  fn !=(a: T, b: T) -> Bool
}

trait Show[T] {
  fn show(value: T) -> String
}

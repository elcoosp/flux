//! Type-class (trait) resolution for `Numeric`, `Eq` and `Show`.
//!
//! The checker needs to confirm that a type brought into scope under a trait
//! bound actually satisfies that trait. For the three prelude traits this is a
//! closed world (spec §18.2): only the primitive scalars and their standard
//! library shapes satisfy them, and `Numeric` additionally requires the type to
//! support the `+` / `-` arithmetic the bound enables.

use crate::kind::TcType;
use flux_syntax::Span;

/// A trait-error diagnostic.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TraitError {
    /// A trait bound could not be satisfied by the concrete type.
    NotSatisfied {
        /// The trait name, e.g. `Numeric`.
        trait_name: String,
        /// The type that failed to satisfy it.
        ty: TcType,
        /// Where the bound was declared.
        span: Span,
    },
}

/// Checks that `ty` satisfies the named trait.
///
/// Returns `Ok(())` when `ty` is a primitive scalar (the only types that
/// satisfy the closed-world prelude traits), or when `ty` still contains
/// inference variables (resolved later at use sites). Component and ADT types
/// are treated as opaque and do not satisfy scalar traits.
///
/// # Errors
///
/// Returns [`TraitError::NotSatisfied`] when a concrete, non-scalar type is
/// asked to satisfy a prelude trait.
pub(crate) fn check_trait_bound(
    trait_name: &str,
    ty: &TcType,
    span: Span,
) -> Result<(), TraitError> {
    match ty {
        TcType::Var(_) => Ok(()),
        // A constrained variable only satisfies a trait when that trait is in
        // its bound list. This is what makes `let x: String = Numeric.zero()`
        // a type error: `String` is not a valid `Numeric` instance.
        TcType::Constrained(_, traits) => {
            if traits.iter().any(|t| t == trait_name) {
                Ok(())
            } else {
                Err(TraitError::NotSatisfied {
                    trait_name: trait_name.to_owned(),
                    ty: ty.clone(),
                    span,
                })
            }
        }
        TcType::Int | TcType::Float => Ok(()),
        TcType::Bool if trait_name == "Eq" || trait_name == "Show" => Ok(()),
        TcType::String if trait_name == "Eq" || trait_name == "Show" => Ok(()),
        _ => Err(TraitError::NotSatisfied {
            trait_name: trait_name.to_owned(),
            ty: ty.clone(),
            span,
        }),
    }
}

/// Whether a binary arithmetic operator (`+`, `-`, `*`, `/`, `%`) is admissible
/// on `ty` under a `Numeric` bound.
///
/// Only `Int` and `Float` admit arithmetic in the prelude; a constrained
/// variable admits it only when `Numeric` is among its trait bounds.
#[must_use]
pub(crate) fn admits_arithmetic(ty: &TcType) -> bool {
    match ty {
        TcType::Int | TcType::Float => true,
        TcType::Var(_) => true,
        _ => check_trait_bound("Numeric", ty, Span::new(0, 0, 0)).is_ok(),
    }
}

/// Whether a comparison operator (`==`, `!=`, `<`, `>`, `<=`, `>=`) is
/// admissible on `ty` under an `Eq` bound.
#[must_use]
pub(crate) fn admits_equality(ty: &TcType) -> bool {
    matches!(
        ty,
        TcType::Int
            | TcType::Float
            | TcType::Bool
            | TcType::String
            | TcType::Var(_)
            | TcType::Constrained(_, _)
    )
}

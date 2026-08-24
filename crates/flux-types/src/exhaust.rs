//! Exhaustiveness checking for `match` over algebraic data types.
//!
//! A `match` on a value of an ADT variant type is exhaustive when its arms
//! cover every variant of the ADT, or when a wildcard (`_`) or wildcard-pattern
//! (`Variant(_, _)`) arm is present that catches the remainder.

use crate::env::{Binding, Env};
use crate::kind::TcType;
use flux_parser::{MatchArm, MatchPatternKind};
use flux_syntax::Span;
use std::collections::HashSet;

/// An exhaustiveness failure.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExhaustivenessError {
    /// The variants that were not covered by any arm.
    pub missing: Vec<String>,
    /// Span of the `match` expression, where the diagnostic points.
    pub span: Span,
}

/// Checks that `scrutinee_ty` (an ADT variant type such as `Shape`) is fully
/// covered by `arms`.
///
/// Returns `Ok(())` when exhaustive. Non-ADT scrutinees (primitives, lists) are
/// considered always-exhaustive here — the spec's exhaustiveness requirement is
/// stated for ADTs. A trailing wildcard or any arm whose pattern is a wildcard
/// is treated as a catch-all.
///
/// # Errors
///
/// Returns [`ExhaustivenessError`] listing the unmatched variant names.
pub(crate) fn check_exhaustive(
    env: &Env,
    scrutinee_ty: &TcType,
    arms: &[MatchArm],
) -> Result<(), ExhaustivenessError> {
    let adt_name = match scrutinee_ty {
        TcType::Variant(name, _) => name.clone(),
        TcType::Named(name, _) => name.clone(),
        _ => return Ok(()),
    };

    if has_catch_all(arms) {
        return Ok(());
    }

    let Some(Binding::Ctor(crate::env::CtorKind::Adt(adt))) = env.lookup(&adt_name) else {
        // Not an ADT we know; nothing to be exhaustive against.
        return Ok(());
    };

    let covered: HashSet<String> = arms
        .iter()
        .filter_map(|arm| variant_name(&arm.pattern.kind))
        .collect();

    let missing: Vec<String> = adt
        .variants
        .iter()
        .map(|v| v.name.clone())
        .filter(|name| !covered.contains(name))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(ExhaustivenessError {
            missing,
            span: arms_span(arms),
        })
    }
}

fn has_catch_all(arms: &[MatchArm]) -> bool {
    arms.iter().any(|arm| match &arm.pattern.kind {
        MatchPatternKind::Wildcard => true,
        MatchPatternKind::Variant { name, fields } => {
            name.name == "_"
                || fields
                    .iter()
                    .all(|f| matches!(f, flux_parser::Pattern::Wildcard(_)))
        }
        MatchPatternKind::Literal(_) | MatchPatternKind::Guard { .. } => false,
        _ => false,
    })
}

fn variant_name(kind: &MatchPatternKind) -> Option<String> {
    match kind {
        MatchPatternKind::Variant { name, .. } => Some(name.name.clone()),
        _ => None,
    }
}

fn arms_span(arms: &[MatchArm]) -> Span {
    match (arms.first(), arms.last()) {
        (Some(first), Some(last)) => Span::new(first.span.file_id, first.span.start, last.span.end),
        _ => Span::new(0, 0, 0),
    }
}

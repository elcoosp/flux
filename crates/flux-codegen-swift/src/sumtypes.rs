//! Emission of Flux algebraic data types (`type Name = A | B(Int)`) and the
//! `match` expressions that consume them, as idiomatic Swift.
//!
//! A Flux sum type lowers to a Swift `enum` with one associated-value `case`
//! per variant. Variant payload fields are positional in Flux, so the codegen
//! names them by their type position. A `match` arm binds the payload with
//! `case let .Variant(binds):`, putting each field directly in scope.

use flux_parser::{MatchPatternKind, TypeDecl};
use flux_syntax::NodeId;

use crate::expressions::render_expr;
use crate::model::swift_type;
use crate::program::Emitter;

/// Emits every algebraic data type in `bridge.types` as a Swift `enum`,
/// preceding the components.
pub(crate) fn emit_sum_types(em: &mut Emitter<'_>) {
    let mut first = true;
    for sum in em.bridge.types() {
        if !first {
            em.push_raw("\n");
        }
        first = false;
        emit_sum_type(em, sum);
    }
}

/// Emits one `type Name = …` as a Swift `enum` with an associated-value `case`
/// per variant.
fn emit_sum_type(em: &mut Emitter<'_>, sum: &TypeDecl) {
    let name = &sum.name.name;
    em.line(0, &format!("enum {name} {{"));
    for variant in &sum.variants {
        let vname = &variant.name.name;
        if variant.fields.is_empty() {
            em.line(1, &format!("case {vname}"));
        } else {
            let params: Vec<String> = variant
                .fields
                .iter()
                .enumerate()
                .map(|(i, t)| format!("field{i}: {}", swift_type(t)))
                .collect();
            em.line(1, &format!("case {vname}({})", params.join(", ")));
        }
    }
    em.line(0, "}");
}

/// Emits a `switch` over an algebraic data type.
///
/// Each `Variant` arm becomes `case let .Variant(binds):` with the payload
/// fields bound directly in scope; a `Wildcard` arm becomes `default`.
pub(crate) fn emit_match(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let flux_parser::ExprKind::Match { scrutinee, arms } = &expr.kind else {
        return;
    };
    let subject = render_expr(scrutinee);
    em.line(indent, &format!("switch {subject} {{"));
    for arm in arms {
        match &arm.pattern.kind {
            MatchPatternKind::Wildcard => {
                em.line(indent + 4, "default:");
                em.emit_expr_body(&arm.body, indent + 8);
            }
            MatchPatternKind::Variant { name, fields } => {
                let binds: Vec<String> = fields
                    .iter()
                    .map(|p| match p {
                        flux_parser::Pattern::Ident(id) => id.name.clone(),
                        flux_parser::Pattern::Wildcard(_) => "_".to_owned(),
                        _ => "_".to_owned(),
                    })
                    .collect();
                em.line(
                    indent + 4,
                    &format!("case let .{}({}):", name.name, binds.join(", ")),
                );
                em.emit_expr_body(&arm.body, indent + 8);
            }
            MatchPatternKind::Literal(lit) => {
                em.line(indent + 4, &format!("case {}:", render_expr(lit)));
                em.emit_expr_body(&arm.body, indent + 8);
            }
            MatchPatternKind::Guard { name, .. } => {
                em.line(indent + 4, &format!("case let .{}(_):", name.name));
                em.emit_expr_body(&arm.body, indent + 8);
            }
            _ => {
                em.line(indent + 4, "default:");
                em.emit_expr_body(&arm.body, indent + 8);
            }
        }
    }
    em.line(indent, "}");
}

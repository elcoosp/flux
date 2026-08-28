//! Emission of Flux algebraic data types (`type Name = A | B(Int)`) and the
//! `match` expressions that consume them, as idiomatic Kotlin.
//!
//! A Flux sum type lowers to a Kotlin `sealed interface` with one `data class`
//! per variant. Variant payload fields are positional in Flux, so the codegen
//! names them `field0`, `field1`, …; a `match` arm smart-casts the scrutinee to
//! the variant and binds each payload field with `val x = scrutinee.fieldN`.

use flux_parser::{MatchPatternKind, TypeDecl};
use flux_syntax::NodeId;

use crate::expressions::render_expr;
use crate::model::kotlin_type;
use crate::program::Emitter;

/// Emits every algebraic data type in `bridge.types` as Kotlin `sealed
/// interface` + `data class` declarations, preceding the components.
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

/// Emits one `type Name = …` as a `sealed interface Name` plus one `data class`
/// per variant.
fn emit_sum_type(em: &mut Emitter<'_>, sum: &TypeDecl) {
    let name = &sum.name.name;
    em.line(0, &format!("sealed interface {name}"));
    for variant in &sum.variants {
        let vname = &variant.name.name;
        if variant.fields.is_empty() {
            em.line(1, &format!("data class {vname} : {name}"));
        } else {
            let params: Vec<String> = variant
                .fields
                .iter()
                .enumerate()
                .map(|(i, t)| format!("val field{i}: {}", kotlin_type(t)))
                .collect();
            em.line(
                1,
                &format!("data class {vname}({}) : {name}", params.join(", ")),
            );
        }
    }
}

/// Emits a `when (scrutinee) { … }` over an algebraic data type.
///
/// Each `Variant` arm becomes `is <Variant> ->` followed by smart-cast bindings
/// for the payload fields, then the arm body; a `Wildcard` arm becomes `else`.
pub(crate) fn emit_match(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let flux_parser::ExprKind::Match { scrutinee, arms } = &expr.kind else {
        return;
    };
    let subject = render_expr(scrutinee);
    em.line(indent, &format!("when ({subject}) {{"));
    for arm in arms {
        match &arm.pattern.kind {
            MatchPatternKind::Wildcard => {
                em.line(indent + 1, "else ->");
                em.emit_expr_body(&arm.body, indent + 2);
            }
            MatchPatternKind::Variant { name, fields } => {
                em.line(indent + 1, &format!("is {} ->", name.name));
                for (i, field) in fields.iter().enumerate() {
                    if let flux_parser::Pattern::Ident(bind) = field {
                        em.line(
                            indent + 2,
                            &format!("val {} = {}.field{i}", bind.name, subject),
                        );
                    }
                }
                em.emit_expr_body(&arm.body, indent + 2);
            }
            MatchPatternKind::Literal(lit) => {
                em.line(indent + 1, &format!("{} ->", render_expr(lit)));
                em.emit_expr_body(&arm.body, indent + 2);
            }
            MatchPatternKind::Guard { name, .. } => {
                em.line(indent + 1, &format!("is {} ->", name.name));
                em.emit_expr_body(&arm.body, indent + 2);
            }
            _ => {
                em.line(indent + 1, "else ->");
                em.emit_expr_body(&arm.body, indent + 2);
            }
        }
    }
    em.line(indent, "}");
}

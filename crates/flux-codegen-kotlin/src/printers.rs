//! Leaf-formatting helpers for the Compose emitter.
//!
//! These are pure functions from a Flux AST fragment to a Kotlin text fragment.
//! Keeping them in their own module keeps [`crate::nodes`] focused on the tree
//! traversal and within the project's 300-line file budget.

use flux_parser::{Expr, ExprKind, MatchPattern};

use crate::expressions::render_expr;

/// Renders a prop value that is a string literal for inline use (Text, Image).
///
/// Bare string literals and interpolations pass through unchanged; this exists
/// so the call sites read declaratively.
#[must_use]
pub(crate) fn render_inline(value: String) -> String {
    value
}

/// Reduces a key function `fn(u) { u.id }` to a Kotlin key extractor
/// `{ it.id }`, else `{ it }` (per Appendix F — stable keys, not positional
/// indices). The lambda parameter names the collection element, so only the
/// field it accesses matters for the key.
#[must_use]
pub(crate) fn key_extractor_of(key: &Expr) -> String {
    if let ExprKind::Lambda { params, body } = &key.kind {
        if let Some(param) = params.first() {
            if let Some(flux_parser::BlockItem::Expr(inner)) = body.items.first() {
                if let ExprKind::Field { base, field } = &inner.kind {
                    if let ExprKind::Ident(base_id) = &base.kind {
                        if base_id.name == param.name.name {
                            return format!("{{ it.{} }}", field.name);
                        }
                    }
                }
            }
        }
    }
    "{ it }".to_owned()
}

/// Renders a match pattern as a Kotlin `when` branch label.
#[must_use]
pub(crate) fn render_pattern(pattern: &MatchPattern) -> String {
    use flux_parser::MatchPatternKind;
    match &pattern.kind {
        MatchPatternKind::Wildcard => "else".to_owned(),
        MatchPatternKind::Variant { name, fields } => {
            let binds: Vec<String> = fields
                .iter()
                .map(|p| match p {
                    flux_parser::Pattern::Ident(id) => id.name.clone(),
                    flux_parser::Pattern::Wildcard(_) => "_".to_owned(),
                    _ => "_".to_owned(),
                })
                .collect();
            if binds.is_empty() {
                name.name.clone()
            } else {
                format!("{}({})", name.name, binds.join(", "))
            }
        }
        MatchPatternKind::Literal(expr) => render_expr(expr),
        MatchPatternKind::Guard { name, .. } => name.name.clone(),
        _ => "else".to_owned(),
    }
}

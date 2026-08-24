//! Leaf-formatting helpers for the SwiftUI emitter.
//!
//! These are pure functions from a Flux AST fragment to a Swift text fragment.
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

/// Reduces a key function `fn(u) { u.id }` to a Swift key-path `\.id`, else
/// `\.self` (per Appendix F — stable keys, not positional indices). The lambda
/// parameter names the collection element, so only the field it accesses
/// matters for the key-path.
#[must_use]
pub(crate) fn key_path_of(key: &Expr) -> String {
    if let ExprKind::Lambda { params, body } = &key.kind {
        if let Some(param) = params.first() {
            if let Some(flux_parser::BlockItem::Expr(inner)) = body.items.first() {
                if let ExprKind::Field { base, field } = &inner.kind {
                    if let ExprKind::Ident(base_id) = &base.kind {
                        if base_id.name == param.name.name {
                            return format!("\\.{}", field.name);
                        }
                    }
                }
            }
        }
    }
    "\\.self".to_owned()
}

/// Renders a match pattern as a Swift `case` label.
#[must_use]
pub(crate) fn render_pattern(pattern: &MatchPattern) -> String {
    use flux_parser::MatchPatternKind;
    match &pattern.kind {
        MatchPatternKind::Wildcard => "_".to_owned(),
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
        _ => "_".to_owned(),
    }
}

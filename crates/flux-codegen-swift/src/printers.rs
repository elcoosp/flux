//! Leaf-formatting helpers for the SwiftUI emitter.
//!
//! These are pure functions from a Flux AST fragment to a Swift text fragment.
//! Keeping them in their own module keeps [`crate::nodes`] focused on the tree
//! traversal and within the project's 300-line file budget.

use flux_parser::{Expr, ExprKind};

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

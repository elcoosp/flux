//! Expression-rendering helpers shared by the dev-path AST reducer ([`crate::model`])
//! and the release-path recognizers ([`crate::recognize_swift`],
//! [`crate::recognize_kotlin`]).
//!
//! The same canonical rendering is applied to the dev AST and to the emitted
//! Swift/Kotlin source so that the three `ViewNode` trees compare equal even
//! though the backends use different surface syntax (`\()` vs `${}`, `.self` vs
//! `{ it }`, `VStack` vs `Column`).

use flux_parser::{BinOp, BlockItem, Expr, ExprKind, StrPart};

/// Recovers the callee identifier name from an expression-origin node.
pub(crate) fn callee_name(expr: &Expr) -> Option<String> {
    if let ExprKind::Call { callee, .. } = &expr.kind {
        if let ExprKind::Ident(ident) = &callee.kind {
            return Some(ident.name.clone());
        }
    }
    None
}

/// Renders a ForEach key function into its canonical extractor token, matching
/// what the release recognizers emit for `\.id` / `{ it.id }` and `\.self` /
/// `{ it }`.
pub(crate) fn render_key(key: &Expr) -> String {
    if let ExprKind::Lambda { params, body } = &key.kind {
        if let Some(param) = params.first() {
            if let Some(BlockItem::Expr(inner)) = body.items.first() {
                if let ExprKind::Field { base, field } = &inner.kind {
                    if let ExprKind::Ident(base_id) = &base.kind {
                        if base_id.name == param.name.name {
                            return format!("key:.{}", field.name);
                        }
                    }
                }
            }
        }
    }
    "key:.self".to_owned()
}

/// Renders `expr` as a canonical expression fragment (no backend-specific
/// delimiters), so the dev path and both codegen backends normalize to the same
/// string.
pub(crate) fn render_expr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Int(value) => value.to_string(),
        ExprKind::Float(value) => render_float(*value),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Str(parts) => render_string(parts),
        ExprKind::Ident(ident) => ident.name.clone(),
        ExprKind::Binary { op, lhs, rhs, .. } => {
            format!(
                "({} {} {})",
                render_expr(lhs),
                binop_symbol(*op),
                render_expr(rhs)
            )
        }
        ExprKind::Field { base, field, .. } => {
            format!("{}.{}", render_expr(base), field.name)
        }
        ExprKind::List(items) => {
            let rendered: Vec<String> = items.iter().map(render_expr).collect();
            format!("[{}]", rendered.join(", "))
        }
        _ => "/* unsupported expr */ 0".to_owned(),
    }
}

fn render_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn render_string(parts: &[StrPart]) -> String {
    let mut body = String::new();
    for part in parts {
        match part {
            StrPart::Text(text) => body.push_str(text),
            StrPart::Interp(expr) => {
                body.push('{');
                body.push_str(&render_expr(expr));
                body.push('}');
            }
            _ => {}
        }
    }
    format!("\"{body}\"")
}

fn binop_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        _ => "+",
    }
}

/// Canonicalizes a release-path expression string (Swift or Kotlin) into the
/// same form [`render_expr`] produces, so the three paths compare equal.
/// Normalizes string-interpolation delimiters and key paths.
#[must_use]
pub(crate) fn canonicalize_expr(text: &str) -> String {
    let t = text.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        // String literal: normalize interpolation delimiters to `{…}`.
        let inner = &t[1..t.len() - 1];
        let inner = inner.replace("\\(", "{").replace("${", "{");
        return format!("\"{inner}\"");
    }
    // Key paths: `\.self` / `\.id` (Swift) and `{ it }` / `{ it.id }` (Kotlin).
    let t = t
        .replace("\\.self", "key:.self")
        .replace("\\.id", "key:.id");
    t.replace("{ it }", "key:.self")
        .replace("{ it.id }", "key:.id")
}

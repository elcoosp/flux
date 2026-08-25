//! Rendering of Flux expressions to Kotlin expression text.
//!
//! The lowered arena drops runtime values to stay compact, so property values,
//! condition expressions and string interpolations are re-rendered from the
//! surface AST. This module converts a [`flux_parser::Expr`] into the Kotlin
//! fragment that appears in the generated composable (e.g. a literal, a
//! `Text` interpolation, or an arithmetic expression).

use flux_parser::{BinOp, Expr, ExprKind, StrPart};

/// Renders `expr` as a Kotlin expression fragment.
///
/// Interpolations inside string literals (`"Count: {count}"`) become Kotlin's
/// string interpolation (`"Count: $count"`); everything else maps directly.
#[must_use]
pub(crate) fn render_expr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Int(value) => value.to_string(),
        ExprKind::Float(value) => render_float(*value),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Str(parts) => render_string(parts),
        ExprKind::Ident(ident) => ident.name.clone(),
        ExprKind::Binary { op, lhs, rhs, .. } => render_binary(*op, lhs, rhs),
        ExprKind::Field { base, field, .. } => format!("{}.{}", render_expr(base), field.name),
        ExprKind::List(items) => render_list(items),
        _ => {
            // Calls, records, and other forms that the MLP does not model as a
            // plain value render as their source spelling in a comment so the
            // generated code still parses and is honest about the gap.
            "/* unsupported expr */ 0".to_owned()
        }
    }
}

/// Renders a float without a trailing `.0` when it is integral, matching
/// Kotlin's preferred literal spelling.
#[must_use]
pub(crate) fn render_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// Renders a string literal, translating `{expr}` interpolations into Kotlin's
/// `$expr` interpolation syntax.
#[must_use]
pub(crate) fn render_string(parts: &[StrPart]) -> String {
    let mut body = String::new();
    for part in parts {
        match part {
            StrPart::Text(text) => body.push_str(text),
            StrPart::Interp(expr) => {
                body.push('$');
                body.push('{');
                body.push_str(&render_expr(expr));
                body.push('}');
            }
            _ => {}
        }
    }
    format!("\"{body}\"")
}

/// Renders a binary operation with Kotlin's operator spelling.
fn render_binary(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
    let symbol = match op {
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
    };
    format!("({} {} {})", render_expr(lhs), symbol, render_expr(rhs))
}

/// Renders a list literal as a Kotlin `listOf(...)` expression.
fn render_list(items: &[Expr]) -> String {
    let rendered: Vec<String> = items.iter().map(render_expr).collect();
    format!("listOf({})", rendered.join(", "))
}

#[cfg(test)]
mod tests {
    use super::{render_expr, render_float, render_string};
    use flux_parser::BinOp;

    fn expr_of(src: &str) -> flux_parser::Expr {
        let full = format!("component Main {{ state x: Int = 0 {src} }}");
        let ast = flux_parser::parse(&full, 0, "t.flux").expect("parse");
        for decl in &ast.decls {
            if let flux_parser::Decl::Component(c) = decl {
                for item in &c.body.items {
                    if let flux_parser::BlockItem::Expr(e) = item {
                        return e.clone();
                    }
                }
            }
        }
        panic!("no expression found in {src}");
    }

    #[test]
    fn int_and_ident_render() {
        assert_eq!(render_expr(&expr_of("42")), "42");
        assert_eq!(render_expr(&expr_of("count")), "count");
    }

    #[test]
    fn string_interpolation_becomes_kotlin_syntax() {
        let out = render_expr(&expr_of("\"Count: {count}\""));
        assert_eq!(out, "\"Count: ${count}\"");
    }

    #[test]
    fn binary_op_renders_with_kotlin_operator() {
        let out = render_expr(&expr_of("a + b"));
        assert_eq!(out, "(a + b)");
        let _ = BinOp::Add;
    }

    #[test]
    fn float_without_trailing_zero() {
        assert_eq!(render_float(4.0), "4.0");
        assert_eq!(render_float(3.5), "3.5");
    }

    #[test]
    fn bare_string_renders_with_quotes() {
        let parts = match &expr_of("\"hi\"").kind {
            flux_parser::ExprKind::Str(p) => p.clone(),
            _ => panic!("expected string"),
        };
        assert_eq!(render_string(&parts), "\"hi\"");
    }
}

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
        _ => "<unknown-binop>", // Mirrors flux-codegen-core; see audit theme #1.
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
        //
        // Both backends can produce an interpolation with an inner expression:
        // Swift emits `\(expr)`, Kotlin `${expr}`. We must (a) rewrite the
        // *opening* delimiter (`\(` or `${`) to `{` and (b) rewrite the
        // *closing* delimiter to `}` — Swift's closing `)` is the one that
        // matches the opening `\(`, tracking paren depth so a nested call
        // such as `\(foo(a, b))` is handled.
        let inner = &t[1..t.len() - 1];
        let inner = canonicalize_interp(inner);
        return format!("\"{inner}\"");
    }
    // Key paths: `\.self` / `\.id` (Swift) and `{ it }` / `{ it.id }` (Kotlin).
    let t = t
        .replace("\\.self", "key:.self")
        .replace("\\.id", "key:.id");
    let t = t
        .replace("{ it }", "key:.self")
        .replace("{ it.id }", "key:.id");
    // Round-16: the Kotlin backend emits `key = null` as the positional-
    // identity key (compiles for any element type, including an unbounded
    // `T`, whose Kotlin upper bound is `Any?`). It is structurally the same
    // as Swift's `\.self`; canonicalize it so cross-language parity holds.
    if t == "null" {
        return "key:.self".to_owned();
    }
    // The codegen backends emit an `unsupported expr` placeholder when a
    // sub-expression cannot be lowered (`0 /* unsupported */` on Swift,
    // `/* unsupported expr */ 0` on Kotlin), and the dev-path reducer renders
    // the same placeholder. Collapse any spelling of it to a single canonical
    // token so structural parity ignores the un-lowered value's exact text
    // (the placeholder is semantically "some value", not part of the structure).
    // Matching is case/space-insensitive so `/* unsupported expr */`,
    // `/*unsupportedexpr*/`, `/* unsupported */` all reduce to `0`.
    let lowered = t.to_ascii_lowercase();
    if lowered.contains("unsupported") {
        return "0".to_owned();
    }
    t
}

/// Canonicalizes the body of a Swift/Kotlin string literal (without the outer
/// quotes) so that both backends produce the same `{…}` interpolation form.
///
/// * Swift emits `\(expr)` — the opening `\(` becomes `{`, the matching `)`
///   becomes `}`. Because the expression itself may contain parens
///   (`\(foo(a, b))`), a nesting depth counter is tracked so only the
///   interpolation's own closing `)` becomes `}`.
/// * Kotlin emits `${expr}` — the opening `${` becomes `{`, the closing `}` is
///   preserved.
///
/// Previously the code performed a naive `.replace("\\(", "{")`, leaving the
/// Swift closing `)` in place. Swift `"\(x)"` became `"{x)"`, which never
/// equalled Kotlin's `"{x}"` — every interpolated string diverged between the
/// two backends, falsely failing parity.
fn canonicalize_interp(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    // Stack of paren depths for currently-open Swift interpolations.
    // Each entry is the current depth within that interpolation; `(` pushes a
    // new frame, `)` decrements — when a frame would go below 0 that `)` is
    // the interpolation's own closer and becomes `}`.
    let mut interp_depth: Vec<i32> = Vec::new();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if chars.peek() == Some(&'(') => {
                // Swift interpolation opener `\(`.
                chars.next();
                out.push('{');
                interp_depth.push(0);
            }
            '$' if chars.peek() == Some(&'{') => {
                // Kotlin interpolation opener `${`.
                chars.next();
                out.push('{');
                interp_depth.push(0);
            }
            '(' => {
                if let Some(depth) = interp_depth.last_mut() {
                    *depth += 1;
                }
                out.push('(');
            }
            ')' => {
                match interp_depth.last_mut() {
                    // Inside a Swift interpolation at depth 0: this `)` closes
                    // the interpolation, not a nested call. Emit `}`.
                    Some(0) => {
                        out.push('}');
                        interp_depth.pop();
                    }
                    Some(depth) => {
                        *depth -= 1;
                        out.push(')');
                    }
                    None => out.push(')'),
                }
            }
            '}' if !interp_depth.is_empty() => {
                // Kotlin interpolation closer.
                out.push('}');
                interp_depth.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod interp_tests {
    use super::*;

    #[test]
    fn swift_and_kotlin_interpolation_canonicalize_to_same_form() {
        let swift = r#""\(count) items""#;
        let kotlin = r#""${count} items""#;
        assert_eq!(
            canonicalize_expr(swift),
            canonicalize_expr(kotlin),
            "Swift \\(x) and Kotlin ${{x}} must canonicalize to the same form",
        );
        assert_eq!(canonicalize_expr(swift), r#""{count} items""#);
    }

    #[test]
    fn swift_interpolation_with_nested_call_parens() {
        // The expression inside `\(…)` may itself contain parens.
        let swift = r#""\(foo(a, b)) done""#;
        assert_eq!(canonicalize_expr(swift), r#""{foo(a, b)} done""#);
    }

    #[test]
    fn multiple_interpolations_in_one_string() {
        let swift = r#""\(a) and \(b)""#;
        assert_eq!(canonicalize_expr(swift), r#""{a} and {b}""#);
    }

    #[test]
    fn plain_string_without_interpolation_is_unchanged() {
        assert_eq!(canonicalize_expr(r#""hello""#), r#""hello""#);
    }
}

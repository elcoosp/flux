//! Rendering of Flux expressions to native expression text (shared, FLUX-047).
//!
//! The lowered arena drops runtime values to stay compact, so property values,
//! condition expressions and string interpolations are re-rendered from the
//! surface AST. The interpolation syntax differs per backend, so the
//! interpolation helper is parameterised by a [`flux_parser::StrPart`] walker
//! supplied by the [`Backend`]. Kotlin turns `{expr}`
//! into `${expr}`; Swift into `\(expr)`.
//!
//! This module is language-neutral: it builds the string body and asks the
//! backend for the placeholder fragment only where a construct cannot be
//! modelled as a plain value.

use flux_parser::{BinOp, Expr, ExprKind, StrPart};

use crate::backend::Backend;

/// Returns `true` when `callee` is the `Router.navigate` call form.
///
/// `Router.navigate(...)` parses as `Field { base: Ident("Router"), field: "navigate" }`
/// — not as a single `Ident` with a dotted name — so we check the field-access shape.
fn is_router_navigate(callee: &Expr) -> bool {
    match &callee.kind {
        ExprKind::Field { base, field, .. } => {
            matches!(&base.kind, ExprKind::Ident(i) if i.name == "Router")
                && field.name == "navigate"
        }
        // Also accept a dotted-ident form that some parser paths may produce.
        ExprKind::Ident(ident) => ident.name == "Router.navigate",
        _ => false,
    }
}

/// Renders `expr` as a native expression fragment.
#[must_use]
pub(crate) fn render_expr<B: Backend>(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Int(value) => value.to_string(),
        ExprKind::Float(value) => render_float(*value),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Str(parts) => render_string::<B>(parts),
        ExprKind::Ident(ident) => ident.name.clone(),
        ExprKind::Binary { op, lhs, rhs, .. } => render_binary::<B>(*op, lhs, rhs),
        ExprKind::Field { base, field, .. } => format!("{}.{}", render_expr::<B>(base), field.name),
        ExprKind::List(items) => render_list::<B>(items),
        _ => {
            // Calls, records, and other forms that the MLP does not model as a
            // plain value render as their source spelling in a comment so the
            // generated code still parses and is honest about the gap.
            B::unsupported_placeholder()
        }
    }
}

/// Renders a float without a trailing `.0` when it is integral.
#[must_use]
pub(crate) fn render_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// Renders a string literal, translating `{expr}` interpolations into the
/// backend's interpolation syntax.
#[must_use]
pub(crate) fn render_string<B: Backend>(parts: &[StrPart]) -> String {
    let mut body = String::new();
    for part in parts {
        match part {
            StrPart::Text(text) => body.push_str(text),
            StrPart::Interp(expr) => {
                body.push_str(B::interp_open());
                body.push_str(&render_expr::<B>(expr));
                body.push_str(B::interp_close());
            }
            _ => {}
        }
    }
    format!("\"{body}\"")
}

/// Renders a binary operation with the native operator spelling (shared: the
/// operator set is identical across Kotlin and Swift).
#[must_use]
fn render_binary<B: Backend>(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
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
    format!(
        "({} {} {})",
        render_expr::<B>(lhs),
        symbol,
        render_expr::<B>(rhs)
    )
}

/// Renders a list literal as a native collection expression.
#[must_use]
fn render_list<B: Backend>(items: &[Expr]) -> String {
    let rendered: Vec<String> = items.iter().map(render_expr::<B>).collect();
    B::list_literal(&rendered)
}

/// Renders one statement of a handler/lambda body.
#[must_use]
fn render_stmt<B: Backend>(stmt: &Expr) -> String {
    match &stmt.kind {
        ExprKind::Assign { target, value } => {
            format!("{} = {}", render_expr::<B>(target), render_expr::<B>(value))
        }
        ExprKind::Await(inner) => format!("await {}", render_expr::<B>(inner)),
        ExprKind::Call { callee, args, .. } => {
            // `Router.navigate("settings")` must become a native navigation
            // push in the release path. The dev VM writes the target to signal
            // 97 (routerActiveChildId); the release path pushes it via the
            // backend's navigation API.
            if is_router_navigate(callee) {
                if let Some(target) = args.first() {
                    let rendered = render_expr::<B>(target.value());
                    return B::router_navigate_expr(&rendered);
                }
            }
            B::unsupported_placeholder()
        }
        _ => render_expr::<B>(stmt),
    }
}

/// Extracts the statement body of an `onClick`/`onTap` handler lambda and
/// renders it as a sequence of native statements. Returns `None` when the
/// handler is absent or empty, so callers can emit a bare `{ }` lambda.
#[must_use]
pub(crate) fn render_handler_body<B: Backend>(handler: &Expr) -> Option<String> {
    let ExprKind::Lambda { body, .. } = &handler.kind else {
        return None;
    };
    let mut out = String::new();
    for item in &body.items {
        if let flux_parser::BlockItem::Expr(stmt) = item {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&render_stmt::<B>(stmt));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

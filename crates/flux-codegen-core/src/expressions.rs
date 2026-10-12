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

/// Returns `true` when `callee` is the `RouterNav.navigate` call form.
///
/// `RouterNav.navigate(...)` parses as `Field { base: Ident("RouterNav"), field: "navigate" }`
/// — not as a single `Ident` with a dotted name — so we check the field-access shape.
fn is_router_navigate(callee: &Expr) -> bool {
    match &callee.kind {
        ExprKind::Field { base, field, .. } => {
            matches!(&base.kind, ExprKind::Ident(i) if i.name == "RouterNav")
                && field.name == "navigate"
        }
        // Also accept a dotted-ident form that some parser paths may produce.
        ExprKind::Ident(ident) => ident.name == "RouterNav.navigate",
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
        // FLUX-F65: a nested call inside an outer call's argument list —
        // `f(g(1))` — previously fell through to the unsupported
        // placeholder, so the inner call was silently replaced with
        // `0 /* unsupported */`. Route `Call` through `render_call` so
        // nested calls render as real native calls. The recursion
        // `render_expr → render_call → render_expr` terminates because
        // each step visits a strictly smaller sub-expression.
        ExprKind::Call { callee, args, .. } => render_call::<B>(callee, args),
        _ => {
            // Records, lambdas, and other forms the MLP does not model as
            // a plain value render as their source spelling in a comment
            // so the generated code still parses and is honest about the
            // gap.
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
pub fn render_string<B: Backend>(parts: &[StrPart]) -> String {
    let mut body = String::new();
    for part in parts {
        match part {
            StrPart::Text(text) => body.push_str(&B::escape_text(text)),
            StrPart::Interp(expr) => {
                body.push_str(B::interp_open());
                body.push_str(&render_expr::<B>(expr));
                body.push_str(B::interp_close());
            }
            // Audit fix: `StrPart` is `#[non_exhaustive]`. The previous wildcard
            // silently dropped the part — an empty string appeared where the
            // user wrote a value, invisible except as a runtime blank. Emit a
            // visible placeholder and `debug_assert!` so the gap is caught in
            // tests rather than shipped.
            _ => {
                debug_assert!(false, "render_string: unhandled StrPart variant");
                body.push_str("/* unhandled StrPart */");
            }
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
        // Audit fix: `BinOp` is `#[non_exhaustive]`; the previous `_ => "+"`
        // silently emitted the wrong operator for any future variant, giving
        // a wrong program with no diagnostic. Match the sentinel used by
        // `view_tree::binop_symbol` and `flux_parity::bridge::binop_symbol`
        // so all three drift-detectors converge: a visibly-broken token that
        // fails to compile rather than a plausible-but-wrong `+`.
        _ => "<unknown-binop>",
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

/// Renders a general call expression `callee(args...)` as native source.
///
/// The callee is rendered as an identifier or a `base.field` path (matching
/// the two shapes `render_expr` already handles); the args are rendered
/// through `render_expr`. Round 34: previously any call inside a handler that
/// was not `RouterNav.navigate(...)` fell through to `unsupported_placeholder`,
/// so `{ handleTap() }`, `{ Api.fetch() }`, and `{ await Cap.method(x) }` all
/// emitted the placeholder instead of the call the user wrote.
#[must_use]
fn render_call<B: Backend>(callee: &Expr, args: &[flux_parser::Arg]) -> String {
    let callee_text = match &callee.kind {
        ExprKind::Ident(i) => i.name.clone(),
        ExprKind::Field { base, field, .. } => {
            format!("{}.{}", render_expr::<B>(base), field.name)
        }
        _ => return B::unsupported_placeholder(),
    };
    // FLUX-F63: preserve the arg's name for `Named` args, using the
    // backend's `named_arg` helper (the same one `render_args` already
    // uses). The previous version dropped the name unconditionally —
    // a Swift `f(n: Int)` called as `f(n: 1)` in Flux emitted `f(1)`,
    // which does not type-check on Swift (parameter labels are part of
    // the call syntax). Kotlin tolerates positional calls to a function
    // with named parameters, but preserving the name is correct there
    // too and matches what the user wrote.
    let arg_texts: Vec<String> = args
        .iter()
        .map(|a| match a {
            flux_parser::Arg::Named { name, value } => {
                B::named_arg(&name.name, &render_expr::<B>(value))
            }
            flux_parser::Arg::Positional(value) => render_expr::<B>(value),
            _ => B::unsupported_placeholder(),
        })
        .collect();
    format!("{}({})", callee_text, arg_texts.join(", "))
}

/// Renders one statement of a handler/lambda body.
#[must_use]
fn render_stmt<B: Backend>(stmt: &Expr) -> String {
    match &stmt.kind {
        ExprKind::Assign { target, value } => {
            format!("{} = {}", render_expr::<B>(target), render_expr::<B>(value))
        }
        ExprKind::Await(inner) => {
            // Audit T-403.8: backend-specific await rendering (Swift await vs
            // Kotlin expr.await()).
            //
            // Round 34: render the awaited expression through `render_call`
            // when it is a call, so the awaited call survives as real source
            // (`Auth.login("u")`) rather than collapsing to the
            // `unsupported_placeholder` fallback (`/* unsupported expr */ 0`).
            // Before this fix, every `await someFn()` handler statement
            // emitted `/* unsupported expr */ 0.await()` (Kotlin) or
            // `await 0 /* unsupported */` (Swift) — i.e. a silent loss of the
            // entire awaited call, no matter which backend.
            let rendered = match &inner.kind {
                ExprKind::Call { callee, args, .. } => render_call::<B>(callee, args),
                _ => render_expr::<B>(inner),
            };
            B::render_await(&rendered)
        }
        ExprKind::Call { callee, args, .. } => {
            // `RouterNav.navigate("settings")` must become a native navigation
            // push in the release path. The dev VM writes the target to signal
            // 97 (routerActiveChildId); the release path pushes it via the
            // backend's navigation API.
            if is_router_navigate(callee) {
                if let Some(target) = args.first() {
                    let rendered = render_expr::<B>(target.value());
                    return B::router_navigate_expr(&rendered);
                }
            }
            // Round 34: render any other call faithfully instead of emitting
            // the `unsupported_placeholder`. Before this fix, a bare call
            // handler (`onPress: { handleTap() }`) was silently replaced with
            // the placeholder, so every tap handler that was a plain call
            // vanished from the generated code. Only the RouterNav branch and
            // the Assign branch reached the emitter with the user's body.
            render_call::<B>(callee, args)
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

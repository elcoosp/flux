//! Hover types over the type-checked AST (FLUX-027).
//!
//! Given the cursor byte offset, finds the innermost expression whose span
//! contains it, maps that expression to its inferred [`TypeKind`] via the
//! `TypedAST` node-id bridge (ADR-0027: expression nodes are keyed by
//! `compute_node_id(0, ExprTag(10), span, None)`), and renders a Markdown
//! hover showing the type. The provider is a pure function of the parsed tree
//! plus the `TypedAST`, so it is directly unit-testable without a socket.

use flux_parser::ast::{Ast, BlockItem, Expr, ExprKind};
use flux_syntax::Span;
use flux_types::TypedAST;

use async_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::util::span_to_range;

/// Returns a hover for the cursor position in `text`, if the cursor sits on an
/// expression with a known type.
#[must_use]
pub(crate) fn hover_at(ast: &Ast, typed: &TypedAST, text: &str, cursor: u32) -> Option<Hover> {
    let expr = innermost_expr(ast, cursor)?;
    let node = flux_syntax::compute_node_id(0, flux_syntax::ExprTag(10), expr, None);
    let ty = typed.type_of(node)?;
    let range = span_to_range(text, expr);
    let markup = MarkupContent {
        kind: MarkupKind::Markdown,
        value: format!("```flux\n{ty}\n```"),
    };
    Some(Hover {
        contents: HoverContents::Markup(markup),
        range: Some(range),
    })
}

/// Finds the innermost expression whose span contains `cursor`.
fn innermost_expr(ast: &Ast, cursor: u32) -> Option<Span> {
    let mut best: Option<Span> = None;
    let mut best_len = u32::MAX;
    for decl in &ast.decls {
        visit_decl(decl, cursor, &mut best, &mut best_len);
    }
    best
}

fn consider(expr: &Expr, cursor: u32, best: &mut Option<Span>, best_len: &mut u32) {
    if expr.span.contains(cursor) && expr.span.len() < *best_len {
        *best = Some(expr.span);
        *best_len = expr.span.len();
    }
}

fn visit_decl(
    decl: &flux_parser::ast::Decl,
    cursor: u32,
    best: &mut Option<Span>,
    best_len: &mut u32,
) {
    match decl {
        flux_parser::ast::Decl::Component(c) => visit_block(&c.body, cursor, best, best_len),
        flux_parser::ast::Decl::Fn(f) => visit_block(&f.body, cursor, best, best_len),
        _ => {}
    }
}

fn visit_block(
    block: &flux_parser::ast::Block,
    cursor: u32,
    best: &mut Option<Span>,
    best_len: &mut u32,
) {
    for item in &block.items {
        match item {
            BlockItem::State(s) => visit_expr(&s.init, cursor, best, best_len),
            BlockItem::Prop { value, .. } => visit_expr(value, cursor, best, best_len),
            BlockItem::Expr(e) => visit_expr(e, cursor, best, best_len),
            _ => {}
        }
    }
}

fn visit_expr(expr: &Expr, cursor: u32, best: &mut Option<Span>, best_len: &mut u32) {
    consider(expr, cursor, best, best_len);
    match &expr.kind {
        ExprKind::Ident(_)
        | ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::Elided => {}
        ExprKind::Str(parts) => {
            for p in parts {
                if let flux_parser::ast::StrPart::Interp(e) = p {
                    visit_expr(e, cursor, best, best_len);
                }
            }
        }
        ExprKind::List(items) => {
            for it in items {
                visit_expr(it, cursor, best, best_len);
            }
        }
        ExprKind::Record { fields, .. } => {
            for (_, value) in fields {
                visit_expr(value, cursor, best, best_len);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            visit_expr(lhs, cursor, best, best_len);
            visit_expr(rhs, cursor, best, best_len);
        }
        ExprKind::Field { base, .. } | ExprKind::OptField { base, .. } => {
            visit_expr(base, cursor, best, best_len);
        }
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            visit_expr(callee, cursor, best, best_len);
            for arg in args {
                visit_expr(arg.value(), cursor, best, best_len);
            }
            if let Some(block) = trailing {
                visit_block(block, cursor, best, best_len);
            }
        }
        ExprKind::Let { value: Some(v), .. } => {
            visit_expr(v, cursor, best, best_len);
        }
        ExprKind::Assign { target, value } => {
            visit_expr(target, cursor, best, best_len);
            visit_expr(value, cursor, best, best_len);
        }
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            visit_expr(cond, cursor, best, best_len);
            visit_block(then_block, cursor, best, best_len);
            if let Some(eb) = else_branch {
                visit_expr(eb, cursor, best, best_len);
            }
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            visit_expr(cond, cursor, best, best_len);
            visit_block(then_block, cursor, best, best_len);
            if let Some(ob) = otherwise {
                visit_block(ob, cursor, best, best_len);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            visit_expr(scrutinee, cursor, best, best_len);
            for arm in arms {
                visit_expr(&arm.body, cursor, best, best_len);
            }
        }
        ExprKind::ForEach { items, key, body } => {
            visit_expr(items, cursor, best, best_len);
            visit_expr(key, cursor, best, best_len);
            visit_block(body, cursor, best, best_len);
        }
        ExprKind::Provide { value, .. } => visit_expr(value, cursor, best, best_len),
        ExprKind::UseContext(_) => {}
        ExprKind::Lambda { body, .. } => visit_block(body, cursor, best, best_len),
        ExprKind::Lifecycle { body, .. } => visit_block(body, cursor, best, best_len),
        ExprKind::Resource(e) => visit_expr(e, cursor, best, best_len),
        ExprKind::Await(e) => visit_expr(e, cursor, best, best_len),
        ExprKind::CreateRef { .. } => {}
        _ => {}
    }
}

// Keep `Expr` import used; the visitor recurses through it.

#[cfg(test)]
mod tests {
    use super::*;
    use flux_parser::parse;

    #[test]
    fn hover_on_integer_literal_shows_int_type() {
        let src = "compo C\n  state n: Int = 0\n  Button(text: \"x\")\n";
        let ast = parse(src, 0, "f.flux").expect("parses");
        let typed = flux_types::type_check(&ast).expect("type-checks");
        // Cursor inside the `0` literal: the type checker infers `Int`.
        let cursor = src.find('0').unwrap() as u32;
        let hover = hover_at(&ast, &typed, src, cursor);
        let got = hover.expect("hover present");
        let HoverContents::Markup(m) = &got.contents else {
            panic!("expected markdown hover");
        };
        assert!(m.value.contains("Int"), "hover was {}", m.value);
    }

    #[test]
    fn no_hover_outside_any_expression() {
        let src = "compo C\n  Button(text: \"x\")\n";
        let ast = parse(src, 0, "f.flux").expect("parses");
        let typed = flux_types::type_check(&ast).expect("type-checks");
        // Cursor on the leading whitespace of the `compo` keyword line: no expr.
        let cursor = 0u32;
        assert!(hover_at(&ast, &typed, src, cursor).is_none());
    }
}

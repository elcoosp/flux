//! AST reducer: lowers a parsed Flux [`Ast`](flux_parser::Ast) into the
//! structural [`ViewNode`] tree used for dev-path parity.
//!
//! State/handler/prop/lifecycle declarations are skipped; only the view graph and
//! control flow are retained. This is the authoritative "what the user wrote" and
//! is exactly what the release codegen derives from.

use flux_parser::{Ast, BlockItem, Decl, Expr, ExprKind};

use crate::bridge::{callee_name, canonicalize_expr, render_expr, render_key};
use crate::model::ViewNode;

/// Builds the structural view tree directly from the parsed AST — the **dev
/// path**'s source of truth.
///
/// The reference VM and differ operate over the lowered reactive IR, but the
/// MLP lowerer ([`flux_ir`]) does not yet lower every B.3 handler/property form.
/// The AST is the authoritative "what the user wrote" and is exactly what the
/// release codegen derives from, so reducing it to the structural [`ViewNode`]
/// tree is the faithful dev-side equivalent. State/handler/prop/lifecycle
/// declarations are skipped; only the view graph and control flow are retained.
#[must_use]
pub fn from_ast(ast: &Ast) -> Vec<ViewNode> {
    let mut roots = Vec::new();
    for decl in &ast.decls {
        if let Decl::Component(component) = decl {
            roots.push(component_from_ast(component));
        }
    }
    roots
}

fn component_from_ast(component: &flux_parser::ComponentDecl) -> ViewNode {
    ViewNode::Component {
        name: component.name.name.clone(),
        children: block_children(&component.body),
    }
}

/// Reduces a block's body items to structural children, skipping `state`/`let`/
/// prop/lifecycle declarations and treating `if`/`when`/`ForEach`/`match`/view
/// calls as structural nodes.
fn block_children(block: &flux_parser::Block) -> Vec<ViewNode> {
    let mut out = Vec::new();
    for item in &block.items {
        match item {
            BlockItem::Expr(expr) => node_from_ast(expr, &mut out),
            BlockItem::State(_) | BlockItem::Prop { .. } => {}
            _ => {}
        }
    }
    out
}

/// Appends the structural node(s) for `expr` to `out`. `if`/`when` may append one
/// or two nodes (a nested `else if` becomes a sibling `if`).
fn node_from_ast(expr: &Expr, out: &mut Vec<ViewNode>) {
    match &expr.kind {
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            let name = callee_name_from_expr(callee);
            let normalized = normalize_view_name(&name);
            match normalized.as_str() {
                "Router" => out.push(ViewNode::Router {
                    children: trailing
                        .as_ref()
                        .map(|b| block_children(b))
                        .unwrap_or_default(),
                }),
                "Screen" => out.push(ViewNode::Screen {
                    route: screen_route_from_args(args),
                    children: trailing
                        .as_ref()
                        .map(|b| block_children(b))
                        .unwrap_or_default(),
                }),
                _ => {
                    let children = trailing
                        .as_ref()
                        .filter(|_| is_container(&normalized))
                        .map(|b| block_children(b))
                        .unwrap_or_default();
                    out.push(ViewNode::Primitive {
                        name: normalized,
                        props: vec![],
                        children,
                    });
                }
            }
        }
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            out.push(ViewNode::If {
                cond: render_cond(cond),
                then_branch: block_children(then_block),
                else_branch: vec![],
            });
            if let Some(else_expr) = else_branch {
                node_from_ast(else_expr, out);
            }
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            // `when … otherwise` lowers to a single combined branch set, mirroring
            // the release codegen's `if … else`.
            out.push(ViewNode::If {
                cond: render_cond(cond),
                then_branch: block_children(then_block),
                else_branch: otherwise
                    .as_ref()
                    .map(|b| block_children(b))
                    .unwrap_or_default(),
            });
        }
        ExprKind::ForEach { items, key, .. } => out.push(ViewNode::ForEach {
            collection: canonicalize_expr(&render_expr(items)),
            key_path: render_key(key),
        }),
        ExprKind::Match {
            scrutinee, arms, ..
        } => out.push(ViewNode::Match {
            scrutinee: canonicalize_expr(&render_expr(scrutinee)),
            arms: arms
                .iter()
                .map(|arm| {
                    let mut body_children = Vec::new();
                    node_from_ast(&arm.body, &mut body_children);
                    (pattern_label(&arm.pattern), body_children)
                })
                .collect(),
        }),
        ExprKind::Lifecycle { .. } => {
            // Lifecycle blocks (`onMount`, `onCleanup`, `effect`, …) are runtime
            // side-effect hooks, not view nodes; they are skipped.
        }
        _ => {}
    }
}

/// Renders a `match` arm pattern to a stable label (variant name, `_`, or the
/// literal text) so arm shapes can be compared across paths.
fn pattern_label(pattern: &flux_parser::MatchPattern) -> String {
    match &pattern.kind {
        flux_parser::MatchPatternKind::Wildcard => "_".to_owned(),
        flux_parser::MatchPatternKind::Variant { name, .. } => name.name.clone(),
        flux_parser::MatchPatternKind::Literal(expr) => canonicalize_expr(&render_expr(expr)),
        flux_parser::MatchPatternKind::Guard { name, .. } => name.name.clone(),
        _ => "<pattern>".to_owned(),
    }
}

/// Recovers the callee name from a callee expression (identifier or field access).
fn callee_name_from_expr(callee: &Expr) -> String {
    match &callee.kind {
        ExprKind::Ident(ident) => ident.name.clone(),
        ExprKind::Field { field, .. } => field.name.clone(),
        other => callee_name(&Expr {
            kind: other.clone(),
            span: callee.span,
        })
        .unwrap_or_else(|| "<anon>".to_owned()),
    }
}

/// Renders a condition expression to its canonical string form.
fn render_cond(cond: &Expr) -> String {
    canonicalize_expr(&render_expr(cond))
}

/// Reads the route literal from a `Screen(route: "...")` call's named args.
fn screen_route_from_args(args: &[flux_parser::Arg]) -> String {
    for arg in args {
        if let flux_parser::Arg::Named { name, value } = arg {
            if name.name == "route" {
                return canonicalize_expr(&render_expr(value));
            }
        }
    }
    String::new()
}

/// Normalizes a codegen backend's view name to the common Flux surface spelling
/// so that Swift `VStack` and Kotlin `Column` compare equal.
#[must_use]
pub fn normalize_view_name(name: &str) -> String {
    match name {
        "VStack" => "Column",
        "HStack" => "Row",
        other => other,
    }
    .to_owned()
}

/// Returns `true` for layout adapters that carry real structural children
/// (their trailing block is part of the view tree). Every other adapter is a
/// leaf: a trailing block in emitted code is a codegen placeholder and must not
/// be recovered as a child.
#[must_use]
pub(crate) fn is_container(name: &str) -> bool {
    matches!(
        name,
        "Column" | "Row" | "VStack" | "HStack" | "ZStack" | "Stack"
    )
}

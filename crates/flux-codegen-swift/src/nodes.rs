//! Per-node SwiftUI renderers for the [`Emitter`](crate::program::Emitter).
//!
//! Each function renders one lowered node kind into SwiftUI, recovering the
//! originating surface expression from the node-ID bridge. These are kept
//! separate from the structural traversal in `program` to stay within the
//! project's 300-line file budget.

use flux_parser::{Block, Expr, ExprKind};
use flux_syntax::NodeId;

use crate::expressions::render_expr;
use crate::model::view_name;
use crate::printers::{key_path_of, render_inline, render_pattern};
use crate::program::Emitter;

/// Emits a primitive view (Text, Button, Image, Column, …) with a deterministic
/// modifier chain derived from its props.
pub(crate) fn emit_primitive(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let node = em.lowered.arena.get(id).expect("primitive node");
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::Call {
        callee,
        args,
        trailing,
    } = &expr.kind
    else {
        return;
    };
    let name = match &callee.kind {
        ExprKind::Ident(ident) => ident.name.clone(),
        _ => return,
    };
    let swift = view_name(&name);
    let mut props = em.collect_props(args, trailing.as_deref());
    // Positional args (e.g. `Text("…")`, `Image(url)`) carry the primary value.
    let positional: Vec<String> = args
        .iter()
        .filter_map(|a| match a {
            flux_parser::Arg::Positional(value) => Some(render_expr(value)),
            _ => None,
        })
        .collect();

    // `Router`/`Screen` lower as primitives in the arena, so route them to the
    // dedicated NavigationStack / destination renderers here.
    if name == "Router" {
        emit_router(em, id, indent);
        return;
    }
    if name == "Screen" {
        emit_screen(em, id, indent);
        return;
    }

    let gap = props.remove("gap");
    if name == "Column" || name == "Row" {
        let spacing = gap.map(|g| format!("(spacing: {g})")).unwrap_or_default();
        em.line(indent, &format!("{swift}{spacing} {{"));
        emit_trailing_or_children(em, trailing.as_deref(), node, indent + 4);
        em.line(indent, "}");
        return;
    }

    // Leaf primitives: Text(value), Button(action:) { … }, Image(…).
    let primary = props
        .get("text")
        .or_else(|| props.get("url"))
        .cloned()
        .or_else(|| positional.first().cloned());
    match name.as_str() {
        "Text" => {
            let value = primary
                .map(render_inline)
                .unwrap_or_else(|| "\"\"".to_owned());
            em.line(indent, &format!("Text({value})"));
        }
        "Button" => {
            em.line(indent, "Button(action: {}) {");
            em.line(indent + 4, "Text(\"\")");
            em.line(indent, "}");
        }
        "Image" => {
            let value = primary
                .map(render_inline)
                .unwrap_or_else(|| "\"\"".to_owned());
            em.line(
                indent,
                &format!("Image(uiImage: UIImage(named: {value}) ?? UIImage())"),
            );
        }
        other => {
            em.line(indent, &format!("{other}()"));
        }
    }
}

/// Emits an `if/else` for a lowered `If` node (covers Flux `if` and
/// `when … otherwise`).
pub(crate) fn emit_if(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let (cond, then_children, else_children): (&Expr, Vec<NodeId>, Vec<NodeId>) = match &expr.kind {
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            let then = em.block_children(then_block);
            let els = match else_branch {
                Some(b) => em.branch_children(b),
                None => Vec::new(),
            };
            (cond, then, els)
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            let then = em.block_children(then_block);
            let els = match otherwise {
                Some(b) => em.block_children(b),
                None => Vec::new(),
            };
            (cond, then, els)
        }
        _ => return,
    };
    let cond_text = render_expr(cond);
    em.line(indent, &format!("if {cond_text} {{"));
    em.emit_children(then_children, indent + 4);
    if !else_children.is_empty() {
        em.line(indent, "} else {");
        em.emit_children(else_children, indent + 4);
    }
    em.line(indent, "}");
}

/// Emits a `ForEach` with a key path (per Appendix F / spec FR-011).
pub(crate) fn emit_for_each(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::ForEach { items, key, body } = &expr.kind else {
        return;
    };
    let collection = render_expr(items);
    let key_path = key_path_of(key);
    em.line(
        indent,
        &format!("ForEach({collection}, id: {key_path}) {{ item in"),
    );
    em.emit_block_body(body, indent + 4);
    em.line(indent, "}");
}

/// Emits a `switch` over an algebraic data type (per spec FR-011).
pub(crate) fn emit_match(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::Match { scrutinee, arms } = &expr.kind else {
        return;
    };
    let subject = render_expr(scrutinee);
    em.line(indent, &format!("switch {subject} {{"));
    for arm in arms {
        let pattern = render_pattern(&arm.pattern);
        em.line(indent + 4, &format!("case {pattern}:"));
        em.emit_expr_body(&arm.body, indent + 8);
    }
    em.line(indent, "}");
}

/// Emits a `NavigationStack` for a `Router` node.
pub(crate) fn emit_router(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let node = em.lowered.arena.get(id).expect("router node");
    em.line(indent, "NavigationStack {");
    em.emit_children(Emitter::child_ids(node), indent + 4);
    em.line(indent, "}");
}

/// Emits a screen as a labelled destination inside the navigation stack.
pub(crate) fn emit_screen(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::Call { args, trailing, .. } = &expr.kind else {
        return;
    };
    let route = args
        .first()
        .map(|a| render_expr(a.value()))
        .unwrap_or_else(|| "\"\"".to_owned());
    em.line(indent, &format!("// Screen route: {route}"));
    if let Some(block) = trailing {
        em.emit_block_body(block, indent);
    }
}

/// Emits the body of a trailing block (component children) at `indent`.
fn emit_trailing_or_children(
    em: &mut Emitter<'_>,
    trailing: Option<&Block>,
    node: flux_ir::NodeView<'_>,
    indent: usize,
) {
    if let Some(block) = trailing {
        em.emit_block_body(block, indent);
    } else {
        em.emit_children(Emitter::child_ids(node), indent);
    }
}
